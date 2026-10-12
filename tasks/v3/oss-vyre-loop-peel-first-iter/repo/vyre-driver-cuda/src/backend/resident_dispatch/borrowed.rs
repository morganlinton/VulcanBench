use std::ffi::c_void;

use smallvec::SmallVec;
use vyre_driver::binding::BindingRole;
use vyre_driver::{BackendError, BindingPlan, DispatchConfig};
use vyre_foundation::ir::Program;

use crate::backend::allocations::{DispatchAllocations, HostTransferAllocations};
use crate::backend::dispatch::CudaBackend;
use crate::backend::ordering::sort_unstable_by_key_if_needed;
use crate::backend::resident::{CudaResidentBuffer, ResidentViewCache};
use crate::backend::resident_dispatch::helpers::{
    next_resident_handle, validate_dense_resident_input_indices, PreparedStep,
};
use crate::backend::resident_dispatch_support::CudaResidentDispatchStep;
use crate::backend::resident_upload_fusion::{
    fuse_resident_upload_copies, push_resident_upload_copy, ResidentUploadCopy,
};
use crate::backend::staging_reserve::{reserve_smallvec, reserved_vec};

pub(super) fn order_resident_fallback_inputs_by_logical_index(
    input_storage: &mut [(usize, Vec<u8>)],
    expected_len: usize,
) -> Result<(), BackendError> {
    sort_unstable_by_key_if_needed(input_storage, |(input_index, _)| *input_index);
    validate_dense_resident_input_indices(
        input_storage.iter().map(|(input_index, _)| *input_index),
        expected_len,
        "resident fallback input storage",
    )
}

impl CudaBackend {
    pub(super) fn resolve_resident_sequence_launch_ptrs(
        &self,
        step: &PreparedStep<'_>,
        resident_view_cache: &mut ResidentViewCache,
    ) -> Result<SmallVec<[u64; 8]>, BackendError> {
        let mut launch_ptrs = SmallVec::<[u64; 8]>::new();
        reserve_smallvec(
            &mut launch_ptrs,
            step.prepared.bindings.bindings.len(),
            "resident sequence launch pointers",
        )?;
        let mut next_handle = 0usize;
        for binding in &step.prepared.bindings.bindings {
            if binding.role == BindingRole::Shared {
                continue;
            }
            let handle =
                next_resident_handle(&step.handles, &mut next_handle, "resident sequence launch")?;
            let resident = self.resident_store.view_cached(
                handle,
                resident_view_cache,
                "resident sequence view cache",
            )?;
            if let Some(expected) = binding.static_byte_len {
                if resident.byte_len < expected {
                    return Err(BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident sequence binding `{}` expected at least {expected} bytes but handle {} has {} bytes.",
                            binding.name, handle.id, resident.byte_len
                        ),
                    });
                }
            }
            if resident.ptr == 0 {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident sequence binding `{}` resolved to a null device pointer; resident launch arguments must preserve descriptor order.",
                        binding.name
                    ),
                });
            }
            launch_ptrs.push(resident.ptr);
        }
        Ok(launch_ptrs)
    }

    pub(crate) fn dispatch_resident_via_borrowed_into(
        &self,
        program: &Program,
        handles: &[CudaResidentBuffer],
        config: &DispatchConfig,
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), BackendError> {
        self.telemetry.record_resident_borrowed_fallback_dispatch();
        let plan = BindingPlan::build(program)?;
        let required_handles = plan
            .bindings
            .len()
            .checked_sub(plan.shared_indices.len())
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA resident fallback binding plan has {} binding(s) but {} shared binding index(es). Rebuild the dispatch plan before launching.",
                    plan.bindings.len(),
                    plan.shared_indices.len()
                ),
            })?;
        if handles.len() != required_handles {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA resident fallback expected {required_handles} resident buffer handle(s) but received {}.",
                    handles.len()
                ),
            });
        }
        let mut input_storage =
            reserved_vec(plan.input_indices.len(), "resident fallback input storage")?;
        let mut output_handles =
            reserved_vec(plan.output_indices.len(), "resident fallback output handle")?;
        let mut next_handle = 0usize;
        for binding in &plan.bindings {
            if binding.role == BindingRole::Shared {
                continue;
            }
            let handle =
                next_resident_handle(handles, &mut next_handle, "resident borrowed dispatch")?;
            if let Some(input_index) = binding.input_index {
                input_storage.push((input_index, self.download_resident(handle)?));
            }
            if let Some(output_index) = binding.output_index {
                output_handles.push((output_index, handle));
            }
        }
        order_resident_fallback_inputs_by_logical_index(
            input_storage.as_mut_slice(),
            plan.input_indices.len(),
        )?;
        let mut input_refs = SmallVec::<[&[u8]; 8]>::new();
        reserve_smallvec(
            &mut input_refs,
            input_storage.len(),
            "resident fallback input reference",
        )?;
        input_refs.extend(input_storage.iter().map(|(_, bytes)| bytes.as_slice()));
        let dispatch_outputs = self.dispatch_borrowed(program, &input_refs, config)?;
        let mut output_uploads = SmallVec::<[(CudaResidentBuffer, &[u8]); 8]>::new();
        reserve_smallvec(
            &mut output_uploads,
            output_handles.len(),
            "resident fallback output upload",
        )?;
        for &(output_index, handle) in &output_handles {
            let output =
                dispatch_outputs
                    .get(output_index)
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident fallback missing output slot {output_index}; keep borrowed dispatch output ordering aligned with BindingPlan."
                        ),
                    })?;
            if !output.is_empty() {
                output_uploads.push((handle, output.as_slice()));
            }
        }
        self.upload_resident_many(&output_uploads)?;
        drop(output_uploads);
        vyre_driver::replace_output_buffers_preserving_slots(dispatch_outputs, outputs);
        Ok(())
    }

    pub(crate) fn dispatch_resident_via_borrowed(
        &self,
        program: &Program,
        handles: &[CudaResidentBuffer],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let mut outputs = reserved_vec(0, "borrowed resident dispatch outputs")?;
        self.dispatch_resident_via_borrowed_into(program, handles, config, &mut outputs)?;
        Ok(outputs)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_resident_param_upload(
        &self,
        param_words: &[u32],
        param_bytes: usize,
        allocation_budget_label: &'static str,
        upload_budget_label: &'static str,
        allocation_metric_label: &'static str,
        upload_metric_label: &'static str,
        allocations: &mut DispatchAllocations,
        host_transfers: &mut HostTransferAllocations,
    ) -> Result<(u64, Option<(u64, *const c_void, usize)>), BackendError> {
        self.validate_transient_allocation_memory_budget(
            param_bytes,
            allocation_budget_label,
            upload_budget_label,
        )?;
        let params_allocation = self.transient_pool.acquire(param_bytes)?;
        self.telemetry.record_transient_allocation_bytes(
            crate::numeric::CUDA_NUMERIC
                .usize_to_u64(params_allocation.byte_len, allocation_metric_label)?,
        );
        let params_ptr = params_allocation.ptr;
        let param_host_ptr = host_transfers.push_u32_words(param_words)?;
        self.telemetry.record_host_to_device_bytes(
            crate::numeric::CUDA_NUMERIC.usize_to_u64(param_bytes, upload_metric_label)?,
        );
        self.telemetry.record_host_upload_operations(1);
        self.telemetry.record_param_upload_bytes(
            crate::numeric::CUDA_NUMERIC.usize_to_u64(param_bytes, upload_metric_label)?,
        );
        allocations.set_params(params_allocation);
        Ok((params_ptr, Some((params_ptr, param_host_ptr, param_bytes))))
    }

    pub(super) fn prepare_resident_sequence_upload_copies<'a>(
        &self,
        uploads: &[(CudaResidentBuffer, &'a [u8])],
    ) -> Result<(SmallVec<[ResidentUploadCopy<'a>; 8]>, u64), BackendError> {
        let mut upload_copies = SmallVec::<[ResidentUploadCopy<'a>; 8]>::new();
        reserve_smallvec(
            &mut upload_copies,
            uploads.len(),
            "resident sequence upload copies",
        )?;
        let mut uploaded_bytes = 0_u64;
        let mut resident_view_cache = ResidentViewCache::new();
        reserve_smallvec(
            &mut resident_view_cache,
            uploads.len(),
            "resident sequence upload view cache",
        )?;
        for &(handle, bytes) in uploads {
            let buffer = self.resident_store.view_cached(
                handle,
                &mut resident_view_cache,
                "resident sequence upload view cache",
            )?;
            if bytes.len() != buffer.byte_len {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident sequence upload for handle {} expected {} bytes but received {}.",
                        handle.id,
                        buffer.byte_len,
                        bytes.len()
                    ),
                });
            }
            push_resident_upload_copy(
                &mut upload_copies,
                &mut uploaded_bytes,
                handle.id,
                buffer.ptr,
                bytes,
                "sequence upload",
            )?;
        }
        fuse_resident_upload_copies(upload_copies)
    }

    pub(super) fn push_prepared_resident_sequence_step<'a>(
        &self,
        step: &'a CudaResidentDispatchStep<'a>,
        prepared_steps: &mut SmallVec<[PreparedStep<'a>; 8]>,
        target_indices: &mut SmallVec<[usize; 16]>,
        all_handles: &mut SmallVec<[CudaResidentBuffer; 32]>,
    ) -> Result<(), BackendError> {
        all_handles.extend(step.handles.iter().copied());
        if let Some(index) = prepared_steps.iter().position(|cached| {
            std::ptr::addr_eq(cached.program, step.program)
                && cached.handles.as_slice() == step.handles
                && cached.config == &step.config
        }) {
            target_indices.push(index);
            return Ok(());
        }
        let prepared = self.prepare_resident_dispatch(step.program, step.handles, &step.config)?;
        let (ptx_src, ptx_source_key) =
            self.ptx_for_program_cached_with_key(step.program, &step.config)?;
        let module_key = self.module_cache_key_for_ptx_source_key(ptx_source_key)?;
        let step_index = prepared_steps.len();
        prepared_steps.push(PreparedStep {
            program: step.program,
            handles: SmallVec::<[CudaResidentBuffer; 8]>::from_slice(step.handles),
            config: &step.config,
            ptx_src,
            module_key,
            prepared,
        });
        target_indices.push(step_index);
        Ok(())
    }
}
