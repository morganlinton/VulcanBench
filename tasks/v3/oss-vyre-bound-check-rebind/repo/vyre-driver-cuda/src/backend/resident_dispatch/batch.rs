use std::ffi::c_void;
use std::sync::Arc;

use rustc_hash::FxHashSet;
use smallvec::SmallVec;
use vyre_driver::binding::BindingRole;
use vyre_driver::{BackendError, DispatchConfig};
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
    checked_resident_dispatch_capacity_mul, CudaResidentBatchDispatch,
};
use crate::backend::staging_reserve::{reserve_hash_set, reserve_smallvec};

impl CudaBackend {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dispatch_resident_batch_async_concrete_with_ptx_key(
        &self,
        program: &Program,
        batches: &[SmallVec<[CudaResidentBuffer; 8]>],
        _config: &DispatchConfig,
        ptx_src: &str,
        module_key: ModuleCacheKey,
        static_params_ptr: Option<u64>,
        prepared: &CudaDispatchPlan,
    ) -> Result<CudaResidentBatchDispatch, BackendError> {
        if batches.is_empty() {
            return Err(BackendError::InvalidProgram {
                fix:
                    "Fix: CUDA resident batch dispatch requires at least one resident handle tuple."
                        .into(),
            });
        }
        self.warmup()?;
        let required_handles = resident_required_handles(prepared)?;
        let batch_handle_capacity = checked_resident_dispatch_capacity_mul(
            batches.len(),
            required_handles,
            "batch handle",
        )?;
        let mut all_handles = SmallVec::<[CudaResidentBuffer; 32]>::new();
        reserve_smallvec(
            &mut all_handles,
            batch_handle_capacity,
            "resident batch all handles",
        )?;
        for (batch_index, handles) in batches.iter().enumerate() {
            if handles.len() != required_handles {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident batch dispatch item {batch_index} expected {required_handles} resident buffer handle(s) but received {}.",
                        handles.len()
                    ),
                });
            }
            all_handles.extend(handles.iter().copied());
        }

        let param_bytes =
            launch_param_byte_len(&prepared.launch.param_words, "resident batch dispatch")?;
        let mut allocations =
            DispatchAllocations::new(program.buffers().len(), Arc::clone(&self.transient_pool))?;
        let mut host_transfers = HostTransferAllocations::with_capacity(
            Arc::clone(&self.host_pool),
            usize::from(static_params_ptr.is_none() && param_bytes != 0),
            0,
        )?;
        let mut param_upload: Option<(u64, *const c_void, usize)> = None;
        let params_ptr = match static_params_ptr {
            Some(ptr) => ptr,
            None if param_bytes == 0 => 0,
            None => {
                let (params_ptr, upload) = self.prepare_resident_param_upload(
                    &prepared.launch.param_words,
                    param_bytes,
                    "CUDA resident batch dispatch parameter bytes",
                    "CUDA resident batch dispatch parameter upload",
                    "resident batch dispatch parameter allocation byte count",
                    "resident batch dispatch parameter upload byte count",
                    &mut allocations,
                    &mut host_transfers,
                )?;
                param_upload = upload;
                params_ptr
            }
        };

        let func = self.resolve_launch_function(
            ptx_src,
            module_key,
            &prepared.launch,
            prepared.cooperative,
        )?;
        let mut output_handles_by_batch = SmallVec::<[SmallVec<[CudaResidentBuffer; 8]>; 8]>::new();
        reserve_smallvec(
            &mut output_handles_by_batch,
            batches.len(),
            "resident batch output handles",
        )?;
        let mut output_readbacks_by_batch =
            SmallVec::<[SmallVec<[CudaOutputReadback; 8]>; 8]>::new();
        reserve_smallvec(
            &mut output_readbacks_by_batch,
            batches.len(),
            "resident batch output readbacks",
        )?;
        let mut launch_ptrs_by_batch = SmallVec::<[SmallVec<[u64; 8]>; 8]>::new();
        reserve_smallvec(
            &mut launch_ptrs_by_batch,
            batches.len(),
            "resident batch launch pointer groups",
        )?;
        let output_binding_count = prepared.output_binding_indices.len();
        let total_output_entries = if output_binding_count == 0 {
            0usize
        } else {
            checked_resident_dispatch_capacity_mul(
                batches.len(),
                output_binding_count,
                "batch output-handle set",
            )?
        };
        let seen_outputs_small = total_output_entries <= 8 && total_output_entries != 0;
        let mut seen_output_handles_small = SmallVec::<[u64; 8]>::new();
        reserve_smallvec(
            &mut seen_output_handles_small,
            total_output_entries.min(8),
            "resident batch small output duplicate set",
        )?;
        let mut seen_output_handles = if !seen_outputs_small && total_output_entries != 0 {
            let mut set = FxHashSet::default();
            reserve_hash_set(
                &mut set,
                total_output_entries,
                "resident batch output duplicate set",
            )?;
            Some(set)
        } else {
            None
        };

        for (batch_index, handles) in batches.iter().enumerate() {
            let mut launch_ptrs = SmallVec::<[u64; 8]>::new();
            reserve_smallvec(
                &mut launch_ptrs,
                prepared.bindings.bindings.len(),
                "resident batch launch pointers",
            )?;
            let mut next_handle = 0usize;
            let mut output_handles_by_index =
                SmallVec::<[(usize, CudaResidentBuffer, CudaOutputReadback); 8]>::new();
            reserve_smallvec(
                &mut output_handles_by_index,
                prepared.output_binding_indices.len(),
                "resident batch output handles by index",
            )?;
            let mut resident_view_cache = ResidentViewCache::new();
            reserve_smallvec(
                &mut resident_view_cache,
                handles.len(),
                "resident batch dispatch view cache",
            )?;
            for binding in &prepared.bindings.bindings {
                if binding.role == BindingRole::Shared {
                    continue;
                }
                let handle =
                    next_resident_handle(handles, &mut next_handle, "resident batch dispatch")?;
                let resident = self.resident_store.view_cached(
                    handle,
                    &mut resident_view_cache,
                    "resident batch dispatch view cache",
                )?;
                if let Some(expected) = binding.static_byte_len {
                    if resident.byte_len < expected {
                        return Err(BackendError::InvalidProgram {
                            fix: format!(
                                "Fix: CUDA resident batch dispatch item {batch_index} binding `{}` expected at least {expected} bytes but handle {} has {} bytes.",
                                binding.name, handle.id, resident.byte_len
                            ),
                        });
                    }
                }
                if resident.ptr == 0 {
                    return Err(BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident batch dispatch item {batch_index} binding `{}` resolved to a null device pointer; resident launch arguments must preserve descriptor order.",
                            binding.name
                        ),
                    });
                }
                launch_ptrs.push(resident.ptr);
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
                        "resident batch output readback",
                    )?;
                    output_handles_by_index.push((output_index, handle, readback));
                }
            }
            if output_handles_by_index.len() != prepared.output_binding_indices.len() {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident batch dispatch item {batch_index} expected {} output handle(s) but resolved {}.",
                        prepared.output_binding_indices.len(),
                        output_handles_by_index.len()
                    ),
                });
            }
            sort_unstable_by_key_if_needed(
                output_handles_by_index.as_mut_slice(),
                |(output_index, _, _)| *output_index,
            );
            validate_dense_resident_output_indices(
                output_handles_by_index
                    .iter()
                    .map(|(output_index, _, _)| *output_index),
                prepared.output_binding_indices.len(),
                "resident batch output handles",
            )?;
            let mut output_handles = SmallVec::<[CudaResidentBuffer; 8]>::new();
            reserve_smallvec(
                &mut output_handles,
                output_handles_by_index.len(),
                "resident batch output handles",
            )?;
            let mut output_readbacks = SmallVec::<[CudaOutputReadback; 8]>::new();
            reserve_smallvec(
                &mut output_readbacks,
                output_handles_by_index.len(),
                "resident batch output readbacks",
            )?;
            for (_, handle, readback) in output_handles_by_index {
                if !seen_outputs_small {
                    if let Some(seen_output_handles) = seen_output_handles.as_mut() {
                        if !seen_output_handles.insert(handle.id) {
                            return Err(BackendError::InvalidProgram {
                                fix: format!(
                                    "Fix: CUDA resident batch dispatch cannot reuse output handle {} across submitted items; allocate one output resident buffer tuple per in-flight batch item so batched readback observes every result instead of the final overwrite.",
                                    handle.id
                                ),
                            });
                        }
                    }
                } else {
                    if seen_output_handles_small.contains(&handle.id) {
                        return Err(BackendError::InvalidProgram {
                            fix: format!(
                                "Fix: CUDA resident batch dispatch cannot reuse output handle {} across submitted items; allocate one output resident buffer tuple per in-flight batch item so batched readback observes every result instead of the final overwrite.",
                                handle.id
                            ),
                        });
                    }
                    seen_output_handles_small.push(handle.id);
                }
                output_handles.push(handle);
                output_readbacks.push(readback);
            }

            if output_handles.len() != prepared.output_binding_indices.len() {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident batch dispatch item {batch_index} expected {} output handle(s) but resolved {}.",
                        prepared.output_binding_indices.len(),
                        output_handles.len()
                    ),
                });
            }
            if output_handles.len() != output_readbacks.len() {
                return Err(BackendError::InvalidProgram {
                    fix: "Fix: CUDA resident batch dispatch output handle/readback stream mismatch after reordering outputs."
                        .into(),
                });
            }

            launch_ptrs_by_batch.push(launch_ptrs);
            output_handles_by_batch.push(output_handles);
            output_readbacks_by_batch.push(output_readbacks);
        }

        let resident_use = self.resident_store.mark_inflight(&all_handles)?;
        let launch_resources = crate::stream::CudaLaunchResourceLease::acquire(
            Arc::clone(&self.launch_resources),
            false,
        )?;
        let mut launch_resources = Some(launch_resources);
        let mut allocations = Some(allocations);
        let mut resident_use = Some(resident_use);
        let mut host_transfers = Some(host_transfers);
        let stream_raw = launch_resources
            .as_ref()
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA resident batch launch resources were consumed before enqueue; rebuild pending dispatch ownership before launching.".to_string(),
            })?
            .stream_raw()?;
        let pending = (|| {
            enqueue_optional_resident_h2d_copy(param_upload, stream_raw)?;

            let mut kernel_args = SmallVec::<[*mut c_void; 8]>::new();
            for launch_ptrs in launch_ptrs_by_batch.iter_mut() {
                let mut params_ref = params_ptr;
                Self::kernel_args_into(launch_ptrs, &mut params_ref, &mut kernel_args)?;
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
            }

            let event = self.launch_resources.acquire_event()?;
            if let Err(error) = event.record(stream_raw) {
                self.launch_resources.release_event(event);
                return Err(error);
            }
            let (stream, _) = launch_resources
                .take()
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: CUDA resident batch launch resources were consumed before pending dispatch ownership transfer.".to_string(),
                })?
                .into_parts()?;
            let allocations = allocations
                .take()
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: CUDA resident batch allocations were consumed before pending dispatch ownership transfer.".to_string(),
                })?;
            let resident_use = resident_use
                .take()
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: CUDA resident batch use guard was consumed before pending dispatch ownership transfer.".to_string(),
                })?;
            let host_transfers =
                host_transfers
                    .take()
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: "Fix: CUDA resident batch host staging was consumed before pending dispatch ownership transfer.".to_string(),
                    })?;
            Ok(
                crate::stream::CudaPendingDispatch::new_resident_batch_pending(
                    Arc::clone(&self.ctx),
                    Arc::clone(&self.launch_resources),
                    event,
                    stream,
                    allocations,
                    resident_use,
                    host_transfers,
                    Arc::clone(&self.telemetry),
                ),
            )
        })();
        let pending = match pending {
            Ok(pending) => pending,
            Err(error) => {
                let Some(launch_resources) = launch_resources.take() else {
                    return Err(error);
                };
                match crate::stream::synchronize_raw_stream(
                    stream_raw,
                    "cuStreamSynchronize (resident batch error cleanup)",
                ) {
                    Ok(()) => {
                        self.telemetry.record_sync_point();
                        return Err(error);
                    }
                    Err(sync_error) => {
                        tracing::error!(
                            "Fix: failed to synchronize CUDA resident batch stream after enqueue error: {sync_error}. In-flight resident batch resources will not be recycled."
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
        };
        Ok(CudaResidentBatchDispatch {
            pending,
            output_handles: output_handles_by_batch,
            output_readbacks: output_readbacks_by_batch,
        })
    }
}
