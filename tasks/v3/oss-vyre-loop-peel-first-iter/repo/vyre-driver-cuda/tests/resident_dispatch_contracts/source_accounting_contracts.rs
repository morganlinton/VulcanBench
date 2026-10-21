use super::*;

#[test]
fn cuda_backend_registration_avoids_collect_staging_at_resident_boundaries() {
    let source = include_str!("../../src/lib.rs");

    assert!(
        source.contains("fn resolve_uploads")
            && source.contains("fn resolve_offset_uploads")
            && source.contains("fn resolve_download_ranges")
            && source.contains("fn resolve_read_ranges"),
        "Fix: CUDA VyreBackend resident API conversion must be centralized in single-pass helpers."
    );
    assert!(
        !source.contains("collect::<SmallVec")
            && !source.contains("resources.push((*resource).clone())")
            && !source.contains("resources.push(range.resource.clone())")
            && !source.contains("resident_handles_from_resources(std::slice::from_ref(resource))")
            && !source.contains("resident_handles_from_resources(std::slice::from_ref(&resource))"),
        "Fix: CUDA resident upload/readback boundaries must not use iterator collect or Resource-clone staging on the release path."
    );
    assert!(
        source.contains("resident_handle_from_resource(resource)?"),
        "Fix: CUDA resident upload/readback boundaries must resolve borrowed Resource handles directly instead of cloning through temporary Resource vectors."
    );
    assert!(
        !source.contains("let mut resources: Vec<Resource> = Vec::with_capacity(resource_capacity);"),
        "Fix: CUDA dispatch_with_device_buffers must dispatch directly on CUDA resident handles instead of building an intermediate Resource Vec."
    );
}

#[test]
fn cuda_resident_readback_preparation_accounts_bytes_without_rescanning_copies() {
    let source = include_str!("../../src/backend/resident_io.rs");
    let fusion_source = include_str!("../../../vyre-driver/src/resident_transfer_fusion.rs");
    let cuda_fusion_source = include_str!("../../src/backend/resident_readback_fusion.rs");

    assert!(
        source.contains("let mut expected_copy_count = 0usize;")
            && source.contains("let mut total_copy_slots = 0usize;")
            && source.contains("download_resident_fused_copies_many_into")
            && source.contains("download_resident_fused_copy_batches_many_into")
            && fusion_source.contains("let mut non_empty_copy_count = 0usize;")
            && fusion_source.contains("let mut bytes = 0u64;"),
        "Fix: CUDA resident readback preparation must accumulate hot-path accounting through the shared fusion helper."
    );
    assert!(
        cuda_fusion_source.contains("type ResidentReadbackCopy = ResidentTransferInterval")
            && cuda_fusion_source
                .contains("type FusedResidentReadbacks = FusedResidentTransfers")
            && cuda_fusion_source.contains("fuse_resident_transfer_intervals(requested)")
            && !cuda_fusion_source.contains("let mut non_empty_copy_count = 0usize;")
            && !cuda_fusion_source.contains("sort_by_key_if_needed"),
        "Fix: CUDA resident readback fusion must remain a thin adapter over vyre-driver interval fusion."
    );
    assert!(
        !source.contains("copies.iter().filter(|copy| copy.byte_len != 0).count()")
            && !source.contains("copies\n            .iter()\n            .fold(0_u64")
            && !source.contains("copy_batches.iter().map(SmallVec::len).sum()")
            && !source.contains("copy_batches.iter().fold(0_u64"),
        "Fix: CUDA resident readback must not rescan prepared copy batches for counts or byte totals."
    );
    assert!(
        source.contains("add_resident_transfer_bytes")
            && source.contains("add_resident_copy_count")
            && source.contains("add_resident_copy_slots")
            && !source.contains(concat!(".", "saturating_add"))
            && !source.contains(concat!("total_memory", "\n            .saturating_mul")),
        "Fix: CUDA resident IO accounting and budget math must be exact/checked, not saturating."
    );
}

#[test]
fn cuda_resident_sequence_upload_accounting_is_single_pass() {
    let source = resident_dispatch_source();
    let fusion_source = include_str!("../../../vyre-driver/src/resident_transfer_fusion.rs");
    let upload_fusion_source = include_str!("../../src/backend/resident_upload_fusion.rs");

    assert!(
        source.contains("push_resident_upload_copy(")
            && source.contains("fuse_resident_upload_copies(upload_copies)")
            && upload_fusion_source.contains("driver_push_resident_upload_copy(")
            && upload_fusion_source.contains("driver_fuse_resident_upload_copies(copies)")
            && fusion_source.contains("add_bytes(uploaded_bytes, bytes.len(), label)?;")
            && fusion_source.contains("let uploaded_bytes = fused_resident_upload_bytes(&fused)?;")
            && fusion_source.contains("fn fused_resident_upload_bytes")
            && fusion_source.contains("let mut uploaded_bytes = 0u64;")
            && fusion_source.contains("add_bytes(&mut uploaded_bytes, copy.bytes.len(), \"fused upload\")?;"),
        "Fix: CUDA resident sequence upload accounting must be accumulated exactly by the shared upload fusion helper."
    );
    assert!(
        source.contains("let fused_readbacks = fuse_resident_readback_copies(&requested_readbacks)?")
            && source.contains("fused_readbacks.non_empty_copy_count")
            && source.contains(".record_device_to_host_readback(fused_readbacks.bytes)")
            && source.contains(".record_device_readback_operations(")
            && source.contains("crate::numeric::CUDA_NUMERIC.usize_to_u64(\n                    fused_readbacks.non_empty_copy_count,")
            && fusion_source.contains("let mut non_empty_copy_count = 0usize;")
            && fusion_source.contains("add_copy_count(non_empty_copy_count")
            && fusion_source.contains("add_bytes(bytes"),
        "Fix: CUDA resident sequence readback accounting must be accumulated exactly by the shared readback fusion helper."
    );
    assert!(
        !source.contains("let uploaded_bytes = upload_copies\n                .iter()\n                .fold(0_u64")
            && !source.contains("let uploaded_bytes = uploads\n                .iter()\n                .fold(0_u64")
            && !source.contains(".filter(|copy| copy.byte_len != 0)\n                    .count()")
            && !source.contains(concat!(".", "saturating_add")),
        "Fix: CUDA resident sequence upload/readback accounting must not rescan uploads or use saturating arithmetic after execution."
    );
}

#[test]
fn cuda_resident_handle_count_uses_binding_plan_cardinality() {
    let source = resident_dispatch_source();

    assert!(
        source.contains("fn resident_required_handles")
            && source.contains(".checked_sub(prepared.bindings.shared_indices.len())"),
        "Fix: CUDA resident dispatch must derive required handle count from BindingPlan cardinalities with checked arithmetic."
    );
    let dispatch_source = include_str!("../../src/backend/dispatch.rs");
    assert!(
        dispatch_source.contains(".checked_sub(static_bindings.shared_indices.len())"),
        "Fix: CUDA resident prepare must derive required handle count from BindingPlan cardinalities with checked arithmetic."
    );
    assert!(
        !source.contains(".filter(|binding| binding.role != BindingRole::Shared)\n            .count()"),
        "Fix: CUDA resident dispatch must not scan bindings just to count non-shared handles before scanning them again for launch pointers."
    );
    assert!(
        !dispatch_source.contains(".filter(|binding| binding.role != BindingRole::Shared)\n            .count()"),
        "Fix: CUDA resident prepare must not scan bindings just to count non-shared handles before scanning them again for input lengths."
    );
}

#[test]
fn cuda_resident_launch_handle_cursors_return_typed_errors() {
    let source = resident_dispatch_source();
    let dispatch_source = include_str!("../../src/backend/dispatch.rs");
    let compiled_dispatch_source = include_str!("../../src/pipeline/compiled_dispatch.rs");

    assert!(
        source.contains("fn next_resident_handle(")
            && source.contains("handles.get(handle_index).copied()")
            && source.contains("ran out of resident buffer handles")
            && source.contains(".checked_add(1)")
            && source.matches("next_resident_handle(").count() >= 5
            && dispatch_source.contains("next_resident_handle(")
            && compiled_dispatch_source.contains("next_resident_handle("),
        "Fix: CUDA resident launch handle cursors must use one shared checked helper that returns BackendError on descriptor drift."
    );
    assert!(
        !source.contains("handles[next_handle]")
            && !source.contains("step.handles[next_handle]")
            && !dispatch_source.contains("handles[next_handle]")
            && !compiled_dispatch_source.contains("handles[next_handle]"),
        "Fix: CUDA resident launch paths must not directly index resident handle slices after cardinality validation."
    );
    assert!(
        source.contains("fn validate_dense_resident_output_indices")
            && source.contains("expected dense {index_kind} indexes 0..{expected_len}")
            && source.matches("validate_dense_resident_output_indices(").count() >= 2
            && source.contains("\"resident dispatch output handles\"")
            && source.contains("\"resident batch output handles\""),
        "Fix: CUDA resident output readback ordering must validate dense output indexes before materializing handles/readbacks."
    );
}

#[test]
fn cuda_resident_dispatch_does_not_allocate_for_empty_launch_params() {
    let source = resident_dispatch_source();

    assert!(
        source.matches("None if param_bytes == 0 => 0").count() >= 2,
        "Fix: CUDA resident single and batched dispatch paths must use a null parameter pointer for empty launch params instead of allocating a rounded 1-byte device buffer."
    );
    assert!(
        source.contains("usize::from(static_params_ptr.is_none() && param_bytes != 0)"),
        "Fix: CUDA resident dispatch must not reserve pinned-host transfer slots when there are no parameter bytes to upload."
    );
}

#[test]
fn cuda_host_dispatch_does_not_allocate_for_empty_launch_params() {
    let source = include_str!("../../src/backend/host_dispatch.rs");

    assert!(
        source.contains("if param_bytes == 0 {\n            0\n        } else {"),
        "Fix: CUDA host dispatch must use a null parameter pointer for empty launch params instead of allocating a rounded 1-byte device buffer."
    );
    assert!(
        source.contains("checked_add_usize_lazy(")
            && source.contains("usize::from(!prepared.launch.param_words.is_empty())"),
        "Fix: CUDA host dispatch must not reserve pinned-host transfer slots when there are no parameter words to upload."
    );
}

#[test]
fn cuda_resident_sequence_preparation_borrows_step_config() {
    let source = resident_dispatch_source();

    assert!(
        source.contains("config: &'a DispatchConfig"),
        "Fix: CUDA resident sequence prepared steps must borrow DispatchConfig instead of cloning per unique sequence step."
    );
    assert!(
        source.contains("config: &step.config"),
        "Fix: CUDA resident sequence preparation must store borrowed step configs."
    );
    assert!(
        !source.contains("config: step.config.clone()"),
        "Fix: CUDA resident sequence preparation must not clone DispatchConfig in the hot path."
    );
}

#[test]
fn cuda_resident_sequence_launch_step_indexes_return_typed_errors() {
    let source = resident_dispatch_source();

    assert!(
        source.contains("resolved_steps.len() != prepared_steps.len()")
            && source.contains("Rebuild the resident sequence launch plan before dispatch")
            && source.contains("prepared_steps.get(step_index)")
            && source.contains("resolved_steps.get_mut(step_index)")
            && source.contains("resident sequence launch references prepared step index")
            && source.contains("resident sequence launch references resolved step index")
            && !source.contains("prepared_steps[step_index]")
            && !source.contains("resolved_steps[step_index]"),
        "Fix: CUDA resident sequence launch must report stale step indexes as BackendError instead of indexing parallel prepared/resolved step tables directly."
    );
}

#[test]
fn cuda_dispatch_wrappers_build_borrowed_inputs_without_iterator_collect() {
    let registration_source = include_str!("../../src/lib.rs");
    let host_dispatch_source = include_str!("../../src/backend/host_dispatch.rs");
    let resident_async_source = include_str!("../../src/backend/resident_dispatch/async_dispatch.rs");
    let resident_batch_source = include_str!("../../src/backend/resident_dispatch/batch.rs");
    let output_range_source = include_str!("../../src/backend/output_range.rs");
    let compiled_dispatch_source = include_str!("../../src/pipeline/compiled_dispatch.rs");
    let plan_source = include_str!("../../src/backend/plan.rs");
    let allocations_source = include_str!("../../src/backend/allocations.rs");

    assert!(
        !registration_source.contains("inputs.iter().map(Vec::as_slice).collect()")
            && !host_dispatch_source.contains("inputs.iter().map(Vec::as_slice).collect()")
            && !compiled_dispatch_source.contains("inputs.iter().map(Vec::as_slice).collect()")
            && !registration_source.contains(concat!(".", "saturating_add")),
        "Fix: CUDA dispatch wrappers must build borrowed input slices with preallocated loops and checked resource capacity, not iterator collect staging or saturating arithmetic."
    );
    assert!(
        !compiled_dispatch_source.contains(".map(|handle| Resource::Resident(handle.id))")
            && !plan_source.contains(".map(|(_, binding_index)| binding_index)"),
        "Fix: CUDA output resource/index conversion must use preallocated loops instead of collect chains."
    );
    assert!(
        host_dispatch_source.contains("let mut upload_bytes = 0_u64;")
            && host_dispatch_source.contains("let mut upload_operations = 0_u64;")
            && host_dispatch_source.contains("add_transfer_bytes(&mut upload_bytes, input.len(), \"host upload\")?")
            && host_dispatch_source
                .contains("add_transfer_operation(&mut upload_operations, \"host upload\")?"),
        "Fix: CUDA host dispatch upload accounting must be accumulated exactly while staging uploads."
    );
    assert!(
        !host_dispatch_source.contains("host_uploads\n            .iter()\n            .fold(0_u64")
            && !host_dispatch_source.contains("host_uploads\n            .iter()\n            .filter(|upload| upload.byte_len != 0)\n            .count()"),
        "Fix: CUDA host dispatch must not rescan staged uploads for telemetry before launch."
    );
    assert!(
        host_dispatch_source.contains("fn host_dispatch_input")
            && host_dispatch_source.contains(".get(input_index)")
            && host_dispatch_source.contains(".copied()")
            && host_dispatch_source.contains("expected input index {input_index}")
            && !host_dispatch_source.contains("inputs[input_index]"),
        "Fix: CUDA host dispatch must turn stale binding input indexes into BackendError instead of directly indexing borrowed input slices."
    );
    assert!(
        output_range_source.contains("fn cuda_output_readback_for_binding")
            && output_range_source.contains(".get(buffer_index)")
            && output_range_source.contains("expected program buffer index {buffer_index}")
            && host_dispatch_source.contains("cuda_output_readback_for_binding(")
            && resident_async_source.contains("cuda_output_readback_for_binding(")
            && resident_batch_source.contains("cuda_output_readback_for_binding(")
            && !host_dispatch_source.contains("fn host_dispatch_buffer")
            && !host_dispatch_source.contains("buffers[binding.buffer_index]")
            && !resident_async_source.contains("program.buffers()[binding.buffer_index]")
            && !resident_batch_source.contains("program.buffers()[binding.buffer_index]"),
        "Fix: CUDA host and resident dispatch readback planning must share one checked program-buffer lookup instead of directly indexing program buffers."
    );
    assert!(
        plan_source.contains("pub(crate) fn output_binding(")
            && plan_source.contains(".bindings.bindings.get(binding_index)")
            && plan_source.contains("expected output binding index {binding_index}")
            && plan_source.contains("without an output index")
            && host_dispatch_source
                .contains("prepared.output_binding(binding_index, \"host dispatch output readback\")?")
            && !host_dispatch_source.contains("prepared.bindings.bindings[binding_index]"),
        "Fix: CUDA host dispatch output binding ordering must use the checked dispatch-plan accessor instead of directly indexing binding descriptors."
    );
    assert!(
        allocations_source.contains("pub(crate) fn set_ptr(")
            && allocations_source.contains(".get_mut(index)")
            && allocations_source.contains("pub(crate) fn ptr(&self, index: usize, context: &str) -> Result<u64, BackendError>")
            && allocations_source.contains("pub(crate) fn byte_len(&self, index: usize, context: &str) -> Result<usize, BackendError>")
            && allocations_source.contains("expected buffer index {index}")
            && !allocations_source.contains("self.ptrs[index]"),
        "Fix: CUDA dispatch allocation-table access must return BackendError for stale buffer indexes instead of panicking on SmallVec indexing."
    );
    assert!(
        host_dispatch_source.contains("checked_add_u64_usize_offset_lazy(")
            && host_dispatch_source.contains("CUDA host dispatch readback device offset")
            && host_dispatch_source.contains("CUDA host dispatch readback pointer overflowed")
            && host_dispatch_source.contains(".checked_add(padded_readback_len)")
            && host_dispatch_source.contains("overflowed while checking capacity")
            && !host_dispatch_source.contains("unwrap_or(usize::MAX)")
            && !host_dispatch_source.contains(".ptr(binding.buffer_index)\n                    .saturating_add(readback.device_offset as u64)")
            && !host_dispatch_source.contains(concat!(".", "saturating_add")),
        "Fix: CUDA host dispatch readback pointer arithmetic and capacity accounting must fail loudly on overflow instead of saturating to a wrong address or capacity."
    );
}
