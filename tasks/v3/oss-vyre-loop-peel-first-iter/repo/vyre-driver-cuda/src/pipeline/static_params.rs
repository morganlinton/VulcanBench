//! Static CUDA launch-parameter upload for compiled pipelines.
//!
//! The parent pipeline module constructs compiled state. This module owns the
//! one device-copy duty for static launch parameters so `src/pipeline.rs`
//! remains orchestration-only.

use vyre_driver::BackendError;

use crate::backend::allocations::{DeviceAllocation, HostTransferAllocations};
use crate::backend::copy::aligned_async_copy_len;
use crate::backend::launch_params::launch_param_byte_len;
use crate::backend::CudaBackend;
use crate::numeric::CUDA_NUMERIC;

enum StaticParamUploadFailure {
    Completed(BackendError),
    CompletionUnproven(BackendError),
}

pub(crate) fn upload_static_launch_params(
    backend: &CudaBackend,
    param_words: &[u32],
) -> Result<DeviceAllocation, BackendError> {
    if param_words.is_empty() {
        return Ok(DeviceAllocation::default());
    }
    let param_bytes = launch_param_byte_len(param_words, "compiled-pipeline static")?;
    backend.validate_transient_allocation_memory_budget(
        param_bytes,
        "CUDA compiled-pipeline static parameter bytes",
        "CUDA compiled-pipeline static parameter upload",
    )?;
    let transfer_bytes = aligned_async_copy_len(param_bytes)?;
    let allocation = backend.transient_pool.acquire(transfer_bytes)?;
    backend
        .telemetry
        .record_transient_allocation_bytes(CUDA_NUMERIC.usize_to_u64(
            allocation.byte_len,
            "static launch parameter allocation byte count",
        )?);
    let mut host_transfers =
        HostTransferAllocations::with_capacity(std::sync::Arc::clone(&backend.host_pool), 1, 0)?;
    let upload_result = (|| {
        let stream = backend
            .launch_resources
            .acquire_stream()
            .map_err(StaticParamUploadFailure::Completed)?;
        let enqueue_result = (|| {
            let param_host_ptr =
                host_transfers.push_u32_words_padded(param_words, transfer_bytes)?;
            // SAFETY: FFI to libcuda.so. Pointer args were validated by the matching
            // alloc / store API; lifetimes are documented in the surrounding function.
            // cuda_check propagates non-success codes as BackendError.
            unsafe {
                crate::backend::copy::h2d_async_checked(
                    allocation.ptr,
                    param_host_ptr,
                    transfer_bytes,
                    stream.raw(),
                )?;
            }
            Ok::<(), BackendError>(())
        })();
        if let Err(error) = enqueue_result {
            match stream.synchronize() {
                Ok(()) => backend.telemetry.record_sync_point(),
                Err(sync_error) => {
                    tracing::error!(
                        "Fix: failed to synchronize CUDA compiled-pipeline static parameter upload stream after enqueue error: {sync_error}. In-flight static parameter upload resources will not be recycled."
                    );
                    std::mem::forget(stream);
                    return Err(StaticParamUploadFailure::CompletionUnproven(error));
                }
            }
            backend.launch_resources.release_stream(stream);
            return Err(StaticParamUploadFailure::Completed(error));
        }
        if let Err(error) = stream.synchronize() {
            tracing::error!(
                "Fix: failed to synchronize CUDA compiled-pipeline static parameter upload stream: {error}. In-flight static parameter upload resources will not be recycled."
            );
            std::mem::forget(stream);
            return Err(StaticParamUploadFailure::CompletionUnproven(error));
        }
        backend.telemetry.record_sync_point();
        backend.launch_resources.release_stream(stream);
        Ok(())
    })();
    match upload_result {
        Ok(()) => {}
        Err(StaticParamUploadFailure::Completed(err)) => {
            backend.transient_pool.release(allocation);
            return Err(err);
        }
        Err(StaticParamUploadFailure::CompletionUnproven(err)) => {
            let _unreleased_allocation = allocation;
            std::mem::forget(host_transfers);
            return Err(err);
        }
    }
    backend.telemetry.record_host_to_device_bytes(
        CUDA_NUMERIC.usize_to_u64(param_bytes, "static launch parameter upload byte count")?,
    );
    backend.telemetry.record_host_upload_operations(1);
    backend.telemetry.record_param_upload_bytes(
        CUDA_NUMERIC.usize_to_u64(param_bytes, "static launch parameter upload byte count")?,
    );
    Ok(allocation)
}
