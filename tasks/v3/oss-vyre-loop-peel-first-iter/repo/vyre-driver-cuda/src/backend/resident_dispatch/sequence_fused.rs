use std::ffi::c_void;
use std::sync::Arc;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use vyre_driver::BackendError;

use crate::backend::allocations::{DispatchAllocations, HostTransferAllocations};
use crate::backend::dispatch::CudaBackend;
use crate::backend::launch_params::launch_param_byte_len;
use crate::backend::output_range::CudaOutputReadback;
use crate::backend::resident::{CudaResidentBuffer, ResidentViewCache};
use crate::backend::resident_dispatch::helpers::{
    enqueue_resident_h2d_copy, enqueue_resident_upload_copies_on_stream,
    prepare_resident_sequence_fills, stage_resident_fill_payload, PreparedStep,
};
use crate::backend::resident_dispatch_support::{
    checked_resident_dispatch_capacity_add, CudaResidentDispatchStep,
};
use crate::backend::resident_io::reserve_borrowed_resident_readback_outputs;
use crate::backend::resident_readback_fusion::{
    fuse_resident_readback_copies, validate_fused_resident_readbacks, ResidentReadbackCopy,
};
use crate::backend::staging_reserve::{reserve_hash_map, reserve_smallvec};

impl CudaBackend {
    pub(crate) fn fill_upload_resident_many_repeated_sequence_read_ranges_borrowed_into(
        &self,
        fills: &[(CudaResidentBuffer, u8)],
        uploads: &[(CudaResidentBuffer, &[u8])],
        prefix_steps: &[CudaResidentDispatchStep<'_>],
        repeated_steps: &[CudaResidentDispatchStep<'_>],
        repeat_count: usize,
        read_handles: &[CudaResidentBuffer],
        readbacks: &[CudaOutputReadback],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
        if read_handles.len() != readbacks.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA resident sequence compact readback expected matching handle/range counts but got {} handle(s) and {} range(s).",
                    read_handles.len(),
                    readbacks.len()
                ),
            });
        }
        if outputs.len() != read_handles.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA resident sequence compact readback expected matching output/range counts but got {} output slot(s) and {} range(s).",
                    outputs.len(),
                    read_handles.len()
                ),
            });
        }
        if fills.is_empty()
            && uploads.is_empty()
            && prefix_steps.is_empty()
            && (repeated_steps.is_empty() || repeat_count == 0)
            && read_handles.is_empty()
        {
            return Ok(());
        }
        if crate::instrumentation::cuda_resident_borrowed_fallback_enabled() {
            tracing::debug!(
                "[cuda-trace] resident repeated sequence using borrowed fallback enabled by {} (release requires {})",
                crate::instrumentation::CUDA_RESIDENT_BORROWED_FALLBACK_ENV,
                crate::instrumentation::CUDA_ALLOW_BORROWED_FALLBACK_ENV
            );
            let mut fill_payload = Vec::new();
            for &(handle, value) in fills {
                let bytes = stage_resident_fill_payload(&mut fill_payload, value, handle.byte_len)?;
                self.upload_resident(handle, bytes)?;
            }
            if !uploads.is_empty() {
                self.upload_resident_many(uploads)?;
            }
            for step in prefix_steps {
                self.dispatch_resident(step.program, step.handles, &step.config)?;
            }
            if repeat_count != 0 {
                for _ in 0..repeat_count {
                    for step in repeated_steps {
                        self.dispatch_resident(step.program, step.handles, &step.config)?;
                    }
                }
            }
            for ((&handle, readback), output) in read_handles
                .iter()
                .zip(readbacks.iter())
                .zip(outputs.iter_mut())
            {
                self.download_resident_range_into(
                    handle,
                    readback.device_offset,
                    readback.byte_len,
                    output,
                )?;
            }
            return Ok(());
        }

        struct ResolvedStep {
            func: cudarc::driver::sys::CUfunction,
            launch_ptrs: SmallVec<[u64; 8]>,
            params_ptr: u64,
        }

        let effective_fills = prepare_resident_sequence_fills(fills, uploads)?;
        let (upload_copies, uploaded_bytes) =
            self.prepare_resident_sequence_upload_copies(uploads)?;
        let effective_repeated_steps_len = if repeat_count == 0 {
            0
        } else {
            repeated_steps.len()
        };
        let prepared_step_capacity = checked_resident_dispatch_capacity_add(
            prefix_steps.len(),
            effective_repeated_steps_len,
            "prepared step",
        )?;
        let mut prepared_steps = SmallVec::<[PreparedStep<'_>; 8]>::new();
        reserve_smallvec(
            &mut prepared_steps,
            prepared_step_capacity,
            "resident sequence prepared steps",
        )?;
        let mut prefix_step_indices = SmallVec::<[usize; 16]>::new();
        reserve_smallvec(
            &mut prefix_step_indices,
            prefix_steps.len(),
            "resident sequence prefix step indices",
        )?;
        let mut repeated_step_indices = SmallVec::<[usize; 16]>::new();
        reserve_smallvec(
            &mut repeated_step_indices,
            effective_repeated_steps_len,
            "resident sequence repeated step indices",
        )?;
        let prefix_step_handle_count = prefix_steps.iter().try_fold(0usize, |total, step| {
            vyre_driver::accounting::checked_add_usize_lazy(
                total,
                step.handles.len(),
                || BackendError::InvalidProgram {
                    fix: "Fix: CUDA resident sequence handle capacity overflowed usize while counting prefix step handles; split the resident sequence."
                        .into(),
                },
            )
        })?;
        let repeated_step_handle_count = if repeat_count == 0 {
            0
        } else {
            repeated_steps.iter().try_fold(0usize, |total, step| {
                vyre_driver::accounting::checked_add_usize_lazy(
                    total,
                    step.handles.len(),
                    || BackendError::InvalidProgram {
                        fix: "Fix: CUDA resident sequence handle capacity overflowed usize while counting repeated step handles; split the resident sequence."
                            .into(),
                    },
                )
            })?
        };
        let step_handle_count = checked_resident_dispatch_capacity_add(
            prefix_step_handle_count,
            repeated_step_handle_count,
            "sequence handle",
        )?;
        let all_handle_capacity = checked_resident_dispatch_capacity_add(
            checked_resident_dispatch_capacity_add(
                checked_resident_dispatch_capacity_add(
                    fills.len(),
                    uploads.len(),
                    "sequence clear/upload handle",
                )?,
                step_handle_count,
                "sequence handle",
            )?,
            read_handles.len(),
            "sequence read-handle",
        )?;
        let mut all_handles = SmallVec::<[CudaResidentBuffer; 32]>::new();
        reserve_smallvec(
            &mut all_handles,
            all_handle_capacity,
            "resident sequence all handles",
        )?;
        all_handles.extend(fills.iter().map(|(handle, _)| *handle));
        all_handles.extend(uploads.iter().map(|(handle, _)| *handle));
        for step in prefix_steps {
            self.push_prepared_resident_sequence_step(
                step,
                &mut prepared_steps,
                &mut prefix_step_indices,
                &mut all_handles,
            )?;
        }
        if repeat_count != 0 {
            for step in repeated_steps {
                self.push_prepared_resident_sequence_step(
                    step,
                    &mut prepared_steps,
                    &mut repeated_step_indices,
                    &mut all_handles,
                )?;
            }
        }
        all_handles.extend(read_handles.iter().copied());

        self.warmup()?;
        let resident_use = self.resident_store.mark_inflight(&all_handles)?;
        let stream = self.launch_resources.acquire_stream()?;
        let mut allocations = SmallVec::<[DispatchAllocations; 8]>::new();
        reserve_smallvec(
            &mut allocations,
            prepared_steps.len(),
            "resident sequence dispatch allocations",
        )?;
        let mut host_transfers = SmallVec::<[HostTransferAllocations; 8]>::new();
        reserve_smallvec(
            &mut host_transfers,
            prepared_steps.len(),
            "resident sequence host transfers",
        )?;
        let mut sequence_param_cache = FxHashMap::<SmallVec<[u32; 8]>, u64>::default();
        reserve_hash_map(
            &mut sequence_param_cache,
            prepared_steps.len(),
            "resident sequence parameter cache",
        )?;
        let mut upload_host_transfers = HostTransferAllocations::with_capacity(
            Arc::clone(&self.host_pool),
            upload_copies.len(),
            0,
        )?;
        let mut readback_host_transfers: Option<HostTransferAllocations> = None;
        // Device active-time capture for the resident sequence. CUDA timing
        // events bracket the kernel launches so telemetry can separate kernel
        // device time from host enqueue/readback overhead in the wall window.
        // Timing is best-effort observability: an exhausted event pool degrades
        // to host-wall-only and must never fail the dispatch itself.
        let dispatch_wall_started = std::time::Instant::now();
        let has_launch_work = !prefix_step_indices.is_empty()
            || (repeat_count != 0 && !repeated_step_indices.is_empty());
        let timing_events = if has_launch_work {
            match self.launch_resources.acquire_timing_event_pair() {
                Ok(pair) => Some(pair),
                Err(error) => {
                    tracing::debug!(
                        "[cuda-trace] resident sequence device timing disabled (event pool exhausted): {error}"
                    );
                    None
                }
            }
        } else {
            None
        };
        let result = (|| {
            let mut sequence_view_cache = ResidentViewCache::new();
            reserve_smallvec(
                &mut sequence_view_cache,
                all_handle_capacity,
                "resident sequence view cache",
            )?;
            for &(handle, value) in &effective_fills {
                let buffer = self.resident_store.view_cached(
                    handle,
                    &mut sequence_view_cache,
                    "resident sequence view cache",
                )?;
                if buffer.byte_len != 0 {
                    // SAFETY: FFI to libcuda.so. Resident pointers are
                    // validated through resident_store.view and marked
                    // in-flight for this sequence before the stream work is
                    // submitted.
                    unsafe {
                        crate::backend::copy::memset_d8_async_checked(
                            buffer.ptr,
                            value,
                            buffer.byte_len,
                            stream.raw(),
                        )?;
                    }
                }
            }
            enqueue_resident_upload_copies_on_stream(
                &upload_copies,
                &mut upload_host_transfers,
                stream.raw(),
            )?;
            let mut resolved_steps = SmallVec::<[ResolvedStep; 8]>::new();
            reserve_smallvec(
                &mut resolved_steps,
                prepared_steps.len(),
                "resident sequence resolved steps",
            )?;
            for step in &prepared_steps {
                let launch_ptrs =
                    self.resolve_resident_sequence_launch_ptrs(step, &mut sequence_view_cache)?;
                let func = self.resolve_launch_function(
                    &step.ptx_src,
                    step.module_key,
                    &step.prepared.launch,
                    step.prepared.cooperative,
                )?;
                let mut step_allocations = DispatchAllocations::new(
                    step.program.buffers().len(),
                    Arc::clone(&self.transient_pool),
                )?;
                let param_bytes = launch_param_byte_len(
                    &step.prepared.launch.param_words,
                    "resident sequence dispatch",
                )?;
                let params_ptr = if param_bytes == 0 {
                    0
                } else if let Some(params_ptr) =
                    sequence_param_cache.get(step.prepared.launch.param_words.as_slice())
                {
                    *params_ptr
                } else {
                    self.validate_transient_allocation_memory_budget(
                        param_bytes,
                        "CUDA resident sequence dispatch parameter bytes",
                        "CUDA resident sequence dispatch parameter upload",
                    )?;
                    let mut step_host_transfers =
                        HostTransferAllocations::with_capacity(Arc::clone(&self.host_pool), 1, 0)?;
                    let params_allocation = self.transient_pool.acquire(param_bytes)?;
                    self.telemetry.record_transient_allocation_bytes(
                        crate::numeric::CUDA_NUMERIC.usize_to_u64(
                            params_allocation.byte_len,
                            "resident sequence parameter allocation byte count",
                        )?,
                    );
                    let params_ptr = params_allocation.ptr;
                    let param_host_ptr =
                        step_host_transfers.push_u32_words(&step.prepared.launch.param_words)?;
                    step_allocations.set_params(params_allocation);
                    allocations.push(step_allocations);
                    host_transfers.push(step_host_transfers);
                    enqueue_resident_h2d_copy(
                        params_ptr,
                        param_host_ptr,
                        param_bytes,
                        stream.raw(),
                    )?;
                    self.telemetry.record_host_to_device_bytes(
                        crate::numeric::CUDA_NUMERIC.usize_to_u64(
                            param_bytes,
                            "resident sequence parameter upload byte count",
                        )?,
                    );
                    self.telemetry.record_host_upload_operations(1);
                    self.telemetry.record_param_upload_bytes(
                        crate::numeric::CUDA_NUMERIC.usize_to_u64(
                            param_bytes,
                            "resident sequence parameter upload byte count",
                        )?,
                    );
                    let mut cached_param_words = SmallVec::<[u32; 8]>::new();
                    reserve_smallvec(
                        &mut cached_param_words,
                        step.prepared.launch.param_words.len(),
                        "resident sequence cached parameter words",
                    )?;
                    cached_param_words.extend_from_slice(&step.prepared.launch.param_words);
                    sequence_param_cache.insert(cached_param_words, params_ptr);
                    params_ptr
                };
                resolved_steps.push(ResolvedStep {
                    func,
                    launch_ptrs,
                    params_ptr,
                });
            }
            if resolved_steps.len() != prepared_steps.len() {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident sequence resolved {} dispatch step(s) for {} prepared step(s). Rebuild the resident sequence launch plan before dispatch.",
                        resolved_steps.len(),
                        prepared_steps.len()
                    ),
                });
            }

            let mut kernel_args = SmallVec::<[*mut c_void; 8]>::new();
            let mut launch_resolved_step = |step_index: usize| -> Result<(), BackendError> {
                let Some(step) = prepared_steps.get(step_index) else {
                    return Err(BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident sequence launch references prepared step index {step_index} but only {} prepared step(s) exist. Rebuild the sequence step index plan before dispatch.",
                            prepared_steps.len()
                        ),
                    });
                };
                let Some(resolved) = resolved_steps.get_mut(step_index) else {
                    return Err(BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident sequence launch references resolved step index {step_index} but only {} resolved step(s) exist. Rebuild the sequence resolved-step table before dispatch.",
                            resolved_steps.len()
                        ),
                    });
                };
                let mut params_ref = resolved.params_ptr;
                Self::kernel_args_into(
                    &mut resolved.launch_ptrs,
                    &mut params_ref,
                    &mut kernel_args,
                )?;
                for _ in 0..step.prepared.fixpoint_iterations {
                    self.launch_prevalidated_function(
                        resolved.func,
                        &mut kernel_args,
                        &step.prepared.launch,
                        stream.raw(),
                        false,
                        step.prepared.cooperative,
                    )?;
                }
                Ok(())
            };

            if let Some((start_event, _)) = timing_events.as_ref() {
                start_event.record(stream.raw())?;
            }
            for &step_index in &prefix_step_indices {
                launch_resolved_step(step_index)?;
            }
            for _ in 0..repeat_count {
                for &step_index in &repeated_step_indices {
                    launch_resolved_step(step_index)?;
                }
            }
            if let Some((_, end_event)) = timing_events.as_ref() {
                end_event.record(stream.raw())?;
            }
            let mut requested_readbacks = SmallVec::<[ResidentReadbackCopy; 8]>::new();
            reserve_smallvec(
                &mut requested_readbacks,
                read_handles.len(),
                "resident sequence requested readbacks",
            )?;
            for (handle, readback) in read_handles.iter().copied().zip(readbacks.iter()) {
                let buffer = self.resident_store.view_cached(
                    handle,
                    &mut sequence_view_cache,
                    "resident sequence view cache",
                )?;
                let end = vyre_driver::accounting::checked_usize_byte_range_end_lazy(
                    readback.device_offset,
                    readback.byte_len,
                    buffer.byte_len,
                    || {
                        BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident sequence compact readback for handle {} overflows usize at offset {} len {}.",
                            handle.id, readback.device_offset, readback.byte_len
                        ),
                    }
                    },
                    |end| {
                        BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident sequence compact readback for handle {} requested bytes [{}..{}) but buffer has {} bytes.",
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
                                "Fix: CUDA resident sequence compact readback device offset {} does not fit CUdeviceptr arithmetic for handle {}.",
                                readback.device_offset, handle.id
                            ),
                        }
                        },
                        || {
                            BackendError::InvalidProgram {
                            fix: format!(
                                "Fix: CUDA resident sequence compact readback pointer arithmetic overflowed for handle {} at offset {}.",
                                handle.id, readback.device_offset
                            ),
                        }
                        },
                    )?
                };
                let copy = ResidentReadbackCopy {
                    handle_id: handle.id,
                    src,
                    byte_len: readback.byte_len,
                };
                requested_readbacks.push(copy);
            }

            let fused_readbacks = fuse_resident_readback_copies(&requested_readbacks)?;
            validate_fused_resident_readbacks(
                &fused_readbacks,
                requested_readbacks.len(),
                "resident sequence compact readback",
            )?;
            reserve_borrowed_resident_readback_outputs(&fused_readbacks.views, outputs)?;

            readback_host_transfers = Some(HostTransferAllocations::with_capacity(
                Arc::clone(&self.host_pool),
                fused_readbacks.non_empty_copy_count,
                fused_readbacks.copies.len(),
            )?);
            let transfers = readback_host_transfers.as_mut().ok_or_else(|| {
                BackendError::InvalidProgram {
                    fix: "Fix: CUDA resident sequence readback staging was not retained for stream-failure cleanup. Recreate the readback staging owner before enqueueing D2H copies.".to_string(),
                }
            })?;
            for copy in &fused_readbacks.copies {
                let dst = transfers.push_output(copy.byte_len)?;
                if copy.byte_len != 0 {
                    // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
                    unsafe {
                        crate::backend::copy::d2h_async_checked(
                            dst,
                            copy.src,
                            copy.byte_len,
                            stream.raw(),
                        )?;
                    }
                }
            }
            stream.synchronize()?;
            self.telemetry.record_sync_point();
            if fused_readbacks.non_empty_copy_count == 0 {
                for output in outputs.iter_mut() {
                    output.clear();
                }
            } else {
                for (output, view) in outputs.iter_mut().zip(fused_readbacks.views.iter()) {
                    transfers.collect_output_range_into(
                        view.copy_slot,
                        view.byte_offset,
                        view.byte_len,
                        *output,
                    )?;
                }
            }
            self.telemetry.record_host_to_device_bytes(uploaded_bytes);
            self.telemetry.record_host_upload_operations(
                crate::numeric::CUDA_NUMERIC
                    .usize_to_u64(upload_copies.len(), "resident host upload operation count")?,
            );
            self.telemetry
                .record_device_to_host_readback(fused_readbacks.bytes);
            self.telemetry.record_device_readback_operations(
                crate::numeric::CUDA_NUMERIC.usize_to_u64(
                    fused_readbacks.non_empty_copy_count,
                    "resident sequence readback operation count",
                )?,
            );
            Ok(())
        })();
        // On the success path the closure synchronized the stream before
        // returning, so both timing events are recorded and complete; read the
        // device interval and record it. A timing-read failure is logged and
        // dropped — observability never converts a successful scan into an error.
        if result.is_ok() {
            if let Some((start_event, end_event)) = timing_events.as_ref() {
                let timing = crate::numeric::CUDA_NUMERIC
                    .elapsed_nanos_u64(
                        dispatch_wall_started,
                        "resident sequence dispatch wall latency",
                    )
                    .and_then(|wall_ns| {
                        start_event
                            .elapsed_time_ns(end_event)
                            .map(|device_ns| (wall_ns, device_ns))
                    });
                match timing {
                    Ok((wall_ns, device_ns)) => {
                        self.telemetry
                            .record_timed_dispatch(wall_ns, Some(device_ns), None, None);
                    }
                    Err(error) => tracing::debug!(
                        "[cuda-trace] resident sequence device timing unavailable: {error}"
                    ),
                }
            }
        }
        if result.is_err() {
            match stream.synchronize() {
                Ok(()) => self.telemetry.record_sync_point(),
                Err(error) => {
                    tracing::error!(
                        "Fix: failed to synchronize CUDA resident sequence stream after an error: {error}. In-flight resident sequence resources will not be recycled."
                    );
                    std::mem::forget(stream);
                    std::mem::forget(resident_use);
                    std::mem::forget(allocations);
                    std::mem::forget(host_transfers);
                    std::mem::forget(upload_host_transfers);
                    std::mem::forget(readback_host_transfers);
                    // The stream failed to synchronize, so its recorded timing
                    // events may still be referenced by in-flight work; forget
                    // them with the other resources instead of recycling.
                    std::mem::forget(timing_events);
                    return result;
                }
            }
        }
        if let Some((start_event, end_event)) = timing_events {
            self.launch_resources.release_timing_event(start_event);
            self.launch_resources.release_timing_event(end_event);
        }
        self.launch_resources.release_stream(stream);
        drop(resident_use);
        result
    }
}
