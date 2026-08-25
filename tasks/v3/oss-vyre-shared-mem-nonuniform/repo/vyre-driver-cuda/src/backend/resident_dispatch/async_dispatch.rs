use std::ffi::c_void;
use std::sync::Arc;

use smallvec::SmallVec;
use vyre_driver::binding::BindingRole;
use vyre_driver::{BackendError, DispatchConfig, PendingDispatch};
use vyre_foundation::ir::Program;

use crate::backend::allocations::{DispatchAllocations, HostTransferAllocations};
use crate::backend::dispatch::CudaBackend;
use crate::backend::launch_params::launch_param_byte_len;
use crate::backend::module_cache::ModuleCacheKey;
use crate::backend::ordering::sort_unstable_by_key_if_needed;
use crate::backend::output_range::{cuda_output_readback_for_binding, CudaOutputReadback};
use crate::backend::plan::CudaDispatchPlan;
use crate::backend::resident::{CudaResidentBuffer, ResidentViewCache};
use crate::backend::resident_dispatch::helpers::{
    enqueue_optional_resident_h2d_copy, next_resident_handle, resident_required_handles,
    validate_dense_resident_output_indices,
};
use crate::backend::resident_dispatch_support::{
    add_resident_dispatch_bytes, add_resident_dispatch_u64_count, CudaResidentDispatch,
};
use crate::backend::staging_reserve::{reserve_smallvec, reserved_vec};

pub(super) fn resident_output_clear_for_readback(
    base_ptr: u64,
    readback: CudaOutputReadback,
    binding_name: &str,
) -> Result<Option<(u64, usize)>, BackendError> {
    if readback.byte_len == 0 {
        return Ok(None);
    }
    let clear_ptr = vyre_driver::accounting::checked_add_u64_usize_offset_lazy(
        base_ptr,
        readback.device_offset,
        || {
            BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA resident output clear offset {} for binding `{binding_name}` does not fit CUdeviceptr arithmetic.",
                readback.device_offset
            ),
        }
        },
        || {
            BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA resident output clear pointer for binding `{binding_name}` overflowed at offset {}.",
                readback.device_offset
            ),
        }
        },
    )?;
    Ok(Some((clear_ptr, readback.byte_len)))
}

impl CudaBackend {
    /// Dispatch a Program asynchronously using caller-provided CUDA-resident buffers.
    pub fn dispatch_resident_async(
        &self,
        program: &Program,
        handles: &[CudaResidentBuffer],
        config: &DispatchConfig,
    ) -> Result<Box<dyn PendingDispatch>, BackendError> {
        if crate::instrumentation::cuda_resident_borrowed_fallback_enabled() {
            let outputs = self.dispatch_resident_via_borrowed(program, handles, config)?;
            return Ok(Box::new(crate::stream::CudaPendingDispatch::new_ready(
                Arc::clone(&self.ctx),
                Arc::clone(&self.launch_resources),
                outputs,
                Arc::clone(&self.telemetry),
            )));
        }
        {
            let prepared = self.prepare_resident_dispatch(program, handles, config)?;
            let (ptx_src, ptx_source_key) =
                self.ptx_for_program_cached_with_key(program, config)?;
            let module_key = self.module_cache_key_for_ptx_source_key(ptx_source_key)?;
            let native = self.dispatch_resident_async_concrete_with_ptx_key(
                program, handles, config, &ptx_src, module_key, false, None, true, &prepared,
            )?;
            return Ok(Box::new(native.pending));
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dispatch_resident_async_concrete_with_ptx_key(
        &self,
        program: &Program,
        handles: &[CudaResidentBuffer],
        _config: &DispatchConfig,
        ptx_src: &str,
        module_key: ModuleCacheKey,
        capture_timing: bool,
        static_params_ptr: Option<u64>,
        capture_outputs: bool,
        prepared: &CudaDispatchPlan,
    ) -> Result<CudaResidentDispatch, BackendError> {
        let _profiler_range =
            crate::profiler::cuda_profiler_range(crate::profiler::CUDA_RESIDENT_DISPATCH_RANGE);
        let trace = crate::instrumentation::cuda_stage_trace_enabled();
        let start = std::time::Instant::now();
        if trace {
            tracing::debug!(
                "[cuda-trace] resident dispatch start buffers={} handles={}",
                program.buffers().len(),
                handles.len()
            );
        }
        self.warmup()?;
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms resident warmup",
                start.elapsed().as_millis()
            );
        }
        let required_handles = resident_required_handles(prepared)?;
        if handles.len() != required_handles {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA resident dispatch expected {required_handles} resident buffer handle(s) but received {}.",
                    handles.len()
                ),
            });
        }
        let mut allocations =
            DispatchAllocations::new(program.buffers().len(), Arc::clone(&self.transient_pool))?;
        let mut launch_ptrs = SmallVec::<[u64; 8]>::new();
        reserve_smallvec(
            &mut launch_ptrs,
            prepared.bindings.bindings.len(),
            "resident dispatch launch pointers",
        )?;
        let mut output_stage_readbacks = SmallVec::<[(u64, CudaOutputReadback); 8]>::new();
        reserve_smallvec(
            &mut output_stage_readbacks,
            if capture_outputs {
                prepared.output_binding_indices.len()
            } else {
                0
            },
            "resident dispatch output staged readbacks",
        )?;
        let mut next_handle = 0usize;
        let mut output_handles_by_index =
            SmallVec::<[(usize, CudaResidentBuffer, CudaOutputReadback, u64); 8]>::new();
        reserve_smallvec(
            &mut output_handles_by_index,
            prepared.output_binding_indices.len(),
            "resident dispatch output handles by index",
        )?;
        let mut output_clears = SmallVec::<[(u64, usize); 8]>::new();
        reserve_smallvec(
            &mut output_clears,
            prepared.output_binding_indices.len(),
            "resident dispatch output clears",
        )?;
        let mut resident_view_cache = ResidentViewCache::new();
        reserve_smallvec(
            &mut resident_view_cache,
            handles.len(),
            "resident dispatch view cache",
        )?;
        for binding in &prepared.bindings.bindings {
            if binding.role == BindingRole::Shared {
                continue;
            }
            let handle =
                next_resident_handle(handles, &mut next_handle, "resident dispatch launch")?;
            let resident = self.resident_store.view_cached(
                handle,
                &mut resident_view_cache,
                "resident dispatch view cache",
            )?;
            if let Some(expected) = binding.static_byte_len {
                if resident.byte_len < expected {
                    return Err(BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident buffer `{}` expected at least {expected} bytes but handle {} has {} bytes.",
                            binding.name, handle.id, resident.byte_len
                        ),
                    });
                }
            }
            if resident.ptr == 0 {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident binding `{}` resolved to a null device pointer; resident launch arguments must preserve descriptor order.",
                        binding.name
                    ),
                });
            }
            let launch_ptr = resident.ptr;
            launch_ptrs.push(launch_ptr);
            if let Some(output_index) = binding.output_index {
                let full_byte_len = match binding.static_byte_len {
                    Some(len) => len,
                    None => resident.byte_len,
                };
                let readback = cuda_output_readback_for_binding(
                    program.buffers(),
                    binding.buffer_index,
                    &binding.name,
                    full_byte_len,
                    "resident async output readback",
                )?;
                output_handles_by_index.push((output_index, handle, readback, launch_ptr));
                if binding.input_index.is_none() {
                    output_clears.extend(resident_output_clear_for_readback(
                        launch_ptr,
                        readback,
                        &binding.name,
                    )?);
                }
            }
        }
        if output_handles_by_index.len() != prepared.output_binding_indices.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA resident dispatch expected {} output handle(s) but resolved {}.",
                    prepared.output_binding_indices.len(),
                    output_handles_by_index.len()
                ),
            });
        }
        sort_unstable_by_key_if_needed(
            output_handles_by_index.as_mut_slice(),
            |(output_index, _, _, _)| *output_index,
        );
        validate_dense_resident_output_indices(
            output_handles_by_index
                .iter()
                .map(|(output_index, _, _, _)| *output_index),
            prepared.output_binding_indices.len(),
            "resident dispatch output handles",
        )?;
        let mut output_handles = SmallVec::<[CudaResidentBuffer; 8]>::new();
        reserve_smallvec(
            &mut output_handles,
            output_handles_by_index.len(),
            "resident dispatch output handles",
        )?;
        let mut output_readbacks = SmallVec::<[CudaOutputReadback; 8]>::new();
        reserve_smallvec(
            &mut output_readbacks,
            output_handles_by_index.len(),
            "resident dispatch output readbacks",
        )?;
        for (_, handle, readback, launch_ptr) in output_handles_by_index {
            output_handles.push(handle);
            output_readbacks.push(readback);
            if capture_outputs {
                output_stage_readbacks.push((launch_ptr, readback));
            }
        }
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms resident args/readbacks launch_ptrs={:x?} output_clears={} output_stage_readbacks={}",
                start.elapsed().as_millis(),
                launch_ptrs,
                output_clears.len(),
                output_stage_readbacks.len()
            );
        }

        let param_bytes = launch_param_byte_len(&prepared.launch.param_words, "resident dispatch")?;
        let mut host_transfers = HostTransferAllocations::with_capacity(
            Arc::clone(&self.host_pool),
            usize::from(static_params_ptr.is_none() && param_bytes != 0),
            output_stage_readbacks.len(),
        )?;
        let mut param_upload: Option<(u64, *const c_void, usize)> = None;
        let params_ptr = match static_params_ptr {
            Some(ptr) => ptr,
            None if param_bytes == 0 => 0,
            None => {
                let (params_ptr, upload) = self.prepare_resident_param_upload(
                    &prepared.launch.param_words,
                    param_bytes,
                    "CUDA resident dispatch parameter bytes",
                    "CUDA resident dispatch parameter upload",
                    "resident dispatch parameter allocation byte count",
                    "resident dispatch parameter upload byte count",
                    &mut allocations,
                    &mut host_transfers,
                )?;
                param_upload = upload;
                params_ptr
            }
        };
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms resident params ptr=0x{params_ptr:x} words={:?} grid={:?} workgroup={:?} element_count={}",
                start.elapsed().as_millis(),
                prepared.launch.param_words,
                prepared.launch.grid,
                prepared.launch.workgroup,
                prepared.launch.element_count
            );
        }

        let resident_use = self.resident_store.mark_inflight(handles)?;
        let launch_resources = crate::stream::CudaLaunchResourceLease::acquire(
            Arc::clone(&self.launch_resources),
            capture_timing,
        )?;
        let mut launch_resources = Some(launch_resources);
        let mut allocations = Some(allocations);
        let mut resident_use = Some(resident_use);
        let mut host_transfers = Some(host_transfers);
        let stream_raw = launch_resources
            .as_ref()
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA resident dispatch launch resources were consumed before enqueue; rebuild launch resource ownership before launching.".to_string(),
            })?
            .stream_raw()?;
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms resident allocations/stream",
                start.elapsed().as_millis()
            );
        }
        let enqueue_result = (|| {
            enqueue_optional_resident_h2d_copy(param_upload, stream_raw)?;
            if trace {
                tracing::debug!(
                    "[cuda-trace] +{}ms resident param upload enqueued",
                    start.elapsed().as_millis()
                );
            }
            for &(dst_ptr, byte_len) in &output_clears {
                // SAFETY: FFI to libcuda.so. Resident output pointers were
                // validated above and byte lengths come from the binding/readback
                // plan. The memset is enqueued on the same stream before launch,
                // matching the borrowed CUDA dispatch output-zeroing contract.
                unsafe {
                    crate::backend::copy::memset_d8_async_checked(
                        dst_ptr, 0, byte_len, stream_raw,
                    )?;
                }
            }
            if trace {
                tracing::debug!(
                    "[cuda-trace] +{}ms resident output clears enqueued",
                    start.elapsed().as_millis()
                );
            }
            if crate::instrumentation::cuda_resident_sync_before_launch_enabled() {
                // SAFETY: stream_raw is owned by launch_resources for the
                // duration of this dispatch. This opt-in diagnostic fence isolates
                // setup copies/memsets from kernel execution without changing the
                // release default.
                crate::stream::synchronize_raw_stream(
                    stream_raw,
                    "cuStreamSynchronize (resident prelaunch)",
                )?;
                self.telemetry.record_sync_point();
                if trace {
                    tracing::debug!(
                        "[cuda-trace] +{}ms resident prelaunch sync complete",
                        start.elapsed().as_millis()
                    );
                }
            }

            if let Some((start_event, _)) = launch_resources
                .as_ref()
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: CUDA resident dispatch launch resources were consumed before timing-event record.".to_string(),
                })?
                .timing_events()?
            {
                start_event.record(stream_raw)?;
            }
            // Fixpoint loop  -  see dispatch_borrowed_async_with_ptx_concrete
            // for the contract. Resolve the CUDA function and argument vector
            // once; fixpoint iterations are kernel replays, not relowering or
            // module-cache lookups.
            let func = self.resolve_launch_function(
                ptx_src,
                module_key,
                &prepared.launch,
                prepared.cooperative,
            )?;
            if trace {
                tracing::debug!(
                    "[cuda-trace] +{}ms resident resolve_launch_function",
                    start.elapsed().as_millis()
                );
            }
            let mut params_ref = params_ptr;
            let mut kernel_args = Self::kernel_args(&mut launch_ptrs, &mut params_ref)?;
            for _ in 0..prepared.fixpoint_iterations {
                self.launch_prevalidated_function(
                    func,
                    &mut kernel_args,
                    &prepared.launch,
                    stream_raw,
                    false,
                    prepared.cooperative,
                )?;
            }
            if let Some((_, end_event)) = launch_resources
                .as_ref()
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: CUDA resident dispatch launch resources were consumed before timing-event record.".to_string(),
                })?
                .timing_events()?
            {
                end_event.record(stream_raw)?;
            }
            // SAFETY: stream_raw is the live CUDA stream used for the launches
            // above. Native resident dispatch intentionally fences after the
            // kernel before host-visible output staging. The direct async DtoH/DtoD
            // path after a resident-staged launch can leave the completion event
            // unsignaled on current CUDA drivers, while an explicit post-kernel
            // fence followed by synchronous readback preserves correctness and
            // keeps the actual Program execution on CUDA instead of falling back
            // to host-buffer dispatch.
            crate::stream::synchronize_raw_stream(
                stream_raw,
                "cuStreamSynchronize (resident post-kernel)",
            )?;
            self.telemetry.record_sync_point();
            if trace {
                tracing::debug!(
                    "[cuda-trace] +{}ms resident post-kernel sync complete",
                    start.elapsed().as_millis()
                );
            }
            Ok(())
        })();
        if let Err(error) = enqueue_result {
            let Some(launch_resources) = launch_resources.take() else {
                return Err(error);
            };
            match crate::stream::synchronize_raw_stream(
                stream_raw,
                "cuStreamSynchronize (resident async error cleanup)",
            ) {
                Ok(()) => {
                    self.telemetry.record_sync_point();
                    return Err(error);
                }
                Err(sync_error) => {
                    tracing::error!(
                        "Fix: failed to synchronize CUDA resident dispatch stream after enqueue error: {sync_error}. In-flight resident dispatch resources will not be recycled."
                    );
                    std::mem::forget(launch_resources);
                    if let Some(allocations) = allocations.take() {
                        std::mem::forget(allocations);
                    }
                    if let Some(resident_use) = resident_use.take() {
                        std::mem::forget(resident_use);
                    }
                    if let Some(host_transfers) = host_transfers.take() {
                        std::mem::forget(host_transfers);
                    }
                    return Err(error);
                }
            }
        }
        let launch_resources = launch_resources
            .take()
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA resident dispatch launch resources were consumed before synchronous output readback.".to_string(),
            })?;
        let allocations = allocations.take().ok_or_else(|| BackendError::InvalidProgram {
            fix: "Fix: CUDA resident dispatch allocations were consumed before synchronous output readback.".to_string(),
        })?;
        let resident_use = resident_use.take().ok_or_else(|| BackendError::InvalidProgram {
            fix: "Fix: CUDA resident dispatch use guard was consumed before synchronous output readback.".to_string(),
        })?;
        let mut host_transfers =
            host_transfers
                .take()
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: CUDA resident dispatch host staging was consumed before synchronous output readback.".to_string(),
                })?;
        let mut staged_readback_bytes = 0_u64;
        let mut staged_readback_ops = 0_u64;
        for &(src_base_ptr, readback) in &output_stage_readbacks {
            let dst = host_transfers.push_output(readback.byte_len)?;
            if readback.byte_len != 0 {
                add_resident_dispatch_bytes(
                    &mut staged_readback_bytes,
                    readback.byte_len,
                    "resident staged output readback",
                )?;
                add_resident_dispatch_u64_count(
                    &mut staged_readback_ops,
                    "resident staged output readback operation",
                )?;
                let src_ptr = vyre_driver::accounting::checked_add_u64_usize_offset_lazy(
                    src_base_ptr,
                    readback.device_offset,
                    || {
                        BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident staged output readback offset {} does not fit CUdeviceptr arithmetic.",
                            readback.device_offset
                        ),
                    }
                    },
                    || BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident staged output pointer overflowed at offset {}.",
                            readback.device_offset
                        ),
                    },
                )?;
                // SAFETY: The source is the transient launch output buffer and
                // the destination is pinned host staging owned by
                // host_transfers. The stream was explicitly synchronized after
                // the kernel above, so a synchronous copy is ordered and
                // cannot strand the completion event behind an async copy that
                // the driver never completes.
                unsafe {
                    crate::backend::copy::d2h_sync_checked_with_label(
                        dst,
                        src_ptr,
                        readback.byte_len,
                        "cuMemcpyDtoH_v2",
                    )?;
                }
            }
        }
        self.telemetry
            .record_device_to_host_readback(staged_readback_bytes);
        self.telemetry
            .record_device_readback_operations(staged_readback_ops);
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms resident launch/output readbacks",
                start.elapsed().as_millis()
            );
        }
        let (stream, timing_events) = launch_resources.into_parts()?;
        let mut outputs = reserved_vec(output_stage_readbacks.len(), "resident staged output")?;
        host_transfers.collect_outputs_into(&mut outputs)?;
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms resident output collection complete",
                start.elapsed().as_millis()
            );
        }
        self.launch_resources.release_stream(stream);
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms resident stream released",
                start.elapsed().as_millis()
            );
        }
        let device_ns = match timing_events.as_ref() {
            Some((start_event, end_event)) => Some(start_event.elapsed_time_ns(end_event)?),
            None => None,
        };
        if let Some((start_event, end_event)) = timing_events {
            self.launch_resources.release_timing_event(start_event);
            self.launch_resources.release_timing_event(end_event);
        }
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms resident timing events released",
                start.elapsed().as_millis()
            );
        }
        drop(resident_use);
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms resident use released",
                start.elapsed().as_millis()
            );
        }
        drop(allocations);
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms resident allocations released",
                start.elapsed().as_millis()
            );
        }
        drop(host_transfers);
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms resident host transfers released",
                start.elapsed().as_millis()
            );
        }
        if trace {
            tracing::debug!(
                "[cuda-trace] +{}ms resident synchronous completion",
                start.elapsed().as_millis()
            );
        }
        Ok(CudaResidentDispatch {
            pending: crate::stream::CudaPendingDispatch::new_ready_timed(
                Arc::clone(&self.ctx),
                Arc::clone(&self.launch_resources),
                outputs,
                device_ns,
                Arc::clone(&self.telemetry),
            ),
            output_handles,
            output_readbacks,
        })
    }
}
