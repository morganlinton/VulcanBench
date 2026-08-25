use super::*;

#[test]
fn wgpu_resident_lifecycle_is_module_owned() {
    let source = resident_resource_source();
    let backend_source = backend_impl_source();
    assert!(
        source.contains("pub(crate) fn allocate_resident(")
            && source.contains("pub(crate) fn free_resident("),
        "resident resource module must own allocation and free helpers"
    );
    assert!(
        source.contains("GpuBufferHandle::alloc")
            && source.contains("backend.resident_handles.insert")
            && source.contains("backend.resident_handles.remove"),
        "resident resource lifecycle must allocate, register, and remove resident handles in one module"
    );
    let allocate_forwarder = backend_source
        .split("fn allocate_resident(")
        .nth(1)
        .and_then(|tail| tail.split("fn upload_resident(").next())
        .expect("Fix: WGPU backend must expose allocate_resident before upload_resident");
    let free_forwarder = backend_source
        .split("fn free_resident(")
        .nth(1)
        .and_then(|tail| tail.split("fn dispatch_resident_timed(").next())
        .expect("Fix: WGPU backend must expose free_resident before dispatch_resident_timed");
    assert!(
        allocate_forwarder.contains("crate::resident_resource::allocate_resident")
            && free_forwarder.contains("crate::resident_resource::free_resident"),
        "backend trait implementation must delegate resident lifecycle to the resident resource module"
    );
    assert!(
        !allocate_forwarder.contains("GpuBufferHandle::alloc")
            && !free_forwarder.contains("resident_handles.remove"),
        "backend_impl.rs must not re-embed resident lifecycle internals"
    );
}

#[test]
fn wgpu_resident_batch_upload_uses_fallible_descriptor_reservation() {
    let source = resident_upload_source();
    let single_body = source
        .split("fn upload_resident(")
        .nth(1)
        .and_then(|tail| tail.split("fn upload_resident_many(").next())
        .expect(
            "Fix: WGPU resident upload module must expose upload_resident before upload_resident_many",
        );
    let batch_body = source
        .split("fn upload_resident_many(")
        .nth(1)
        .and_then(|tail| tail.split("fn upload_resident_at(").next())
        .expect("Fix: WGPU resident upload module must expose upload_resident_many before upload_resident_at");
    assert!(
        single_body.contains("upload_resident_many(backend, &[(resource, bytes)])")
            && !single_body.contains("backend.resident_handles.get")
            && !single_body.contains("crate::buffer::write_padded"),
        "single resident upload must delegate to the batch path instead of duplicating validation and staging internals"
    );
    assert!(
        batch_body.contains("try_reserve_smallvec_to_capacity(&mut resolved, uploads.len())"),
        "resident batch upload must reserve validated descriptor storage fallibly"
    );
    assert!(
        !batch_body.contains("with_capacity(uploads.len())"),
        "resident batch upload must not use infallible descriptor allocation in the hot path"
    );
}

#[test]
fn wgpu_resident_download_constructors_use_fallible_output_reservation() {
    let source = resident_download_source();
    let full_body = source
        .split("fn download_resident(")
        .nth(1)
        .and_then(|tail| tail.split("fn download_resident_into(").next())
        .expect(
            "Fix: WGPU resident download module must expose download_resident before download_resident_into",
        );
    let range_body = source
        .split("fn download_resident_range(")
        .nth(1)
        .and_then(|tail| tail.split("fn download_resident_range_into(").next())
        .expect(
            "Fix: WGPU resident download module must expose download_resident_range before download_resident_range_into",
        );
    assert!(
        full_body.contains("try_reserve_vec_to_capacity(&mut bytes, allocation_len)"),
        "full resident download must reserve output storage fallibly"
    );
    assert!(
        range_body.contains("try_reserve_vec_to_capacity(&mut bytes, byte_len)"),
        "ranged resident download must reserve output storage fallibly"
    );
    assert!(
        !full_body.contains("Vec::with_capacity(allocation_len)")
            && !range_body.contains("Vec::with_capacity(byte_len)"),
        "resident download constructors must not use infallible output allocation"
    );
}

#[test]
fn wgpu_buffer_handle_readback_reserves_before_clearing_caller_output() {
    let source = buffer_handle_source();
    let readback = source
        .split("pub(crate) fn readback_range_until(")
        .nth(1)
        .and_then(|tail| tail.split("/// Stable process-local handle id").next())
        .expect("Fix: WGPU buffer handle must expose readback_range_until before handle identity helpers.");
    let reserve_pos = readback
        .find("out.try_reserve_exact(additional)")
        .expect("Fix: WGPU buffer handle readback must reserve caller output storage fallibly.");
    let clear_pos = readback
        .find("out.clear();\n            out.extend_from_slice(visible);")
        .expect("Fix: WGPU buffer handle readback must clear and rewrite caller output after successful reservation.");

    assert!(
        reserve_pos < clear_pos,
        "Fix: shared WGPU handle readback must reserve before clearing caller output so full, ranged, resident, and pipeline readbacks stay transactional on allocation failure."
    );
}

#[test]
fn wgpu_recorded_readback_reserves_before_clearing_caller_output() {
    let source = record_and_readback_source();
    let collector = source
        .split("pub(crate) fn collect_after_submission_wait_timed(")
        .nth(1)
        .expect("Fix: WGPU record-and-readback collector must expose timed collection.");
    let reserve_pos = collector
        .find("try_reserve_vec_to_capacity(out, read_len)")
        .expect(
            "Fix: WGPU recorded readback collector must reserve caller output storage fallibly.",
        );
    let clear_pos = collector
        .find("out.clear();\n                        out.extend_from_slice(bytes);")
        .expect("Fix: WGPU recorded readback collector must clear and rewrite caller output after successful reservation.");

    assert!(
        reserve_pos < clear_pos,
        "Fix: direct WGPU recorded readback must reserve before clearing caller output so allocation failure cannot clobber existing bytes."
    );
}

#[test]
fn wgpu_readback_ring_collect_reserves_before_clearing_caller_output() {
    let source = readback_ring_source();
    let collector = source
        .split("fn copy_ready_slot_into(")
        .nth(1)
        .and_then(|tail| tail.split("#[inline]").next())
        .expect("Fix: WGPU readback ring must expose ready-slot collection before slot traversal helpers.");
    let reserve_pos = collector
        .find("out.try_reserve_exact(additional)")
        .expect("Fix: WGPU readback ring collection must reserve caller output storage fallibly.");
    let clear_pos = collector
        .find("out.clear();\n                out.extend_from_slice(bytes);")
        .expect("Fix: WGPU readback ring collection must clear and rewrite caller output after successful reservation.");

    assert!(
        reserve_pos < clear_pos,
        "Fix: WGPU readback ring collection must reserve before clearing caller output so ring-backed dispatches stay transactional on allocation failure."
    );
}

#[test]
fn wgpu_resident_batch_download_uses_shared_interval_fusion() {
    let source = resident_download_source();
    let batch_body = source
        .split("fn download_resident_ranges_into(")
        .nth(1)
        .and_then(|tail| tail.split("fn copy_fused_resident_view_into(").next())
        .expect("Fix: WGPU resident download module must expose batch download before fused output materialization.");
    assert!(
        source.contains("vyre_driver::resident_transfer_fusion")
            && batch_body.contains("fuse_resident_transfer_intervals(&copies)?")
            && batch_body.contains("reserve_fused_resident_view_outputs(&fused.copies, &fused.views, outputs)?")
            && batch_body.contains("handles.sort_unstable_by_key")
            && batch_body.contains("handles.dedup_by_key")
            && batch_body.contains("for copy in fused.copies.iter().copied()")
            && batch_body.contains("handles\n            .binary_search_by_key")
            && batch_body.contains("for (view, output) in fused.views.iter().copied().zip(outputs.iter_mut())")
            && !batch_body.contains("for ((handle, byte_offset, byte_len), output)"),
        "Fix: WGPU resident ranged batch download must share CUDA's backend-neutral interval fusion instead of issuing one readback per requested range."
    );
}

#[test]
fn wgpu_resident_fused_batch_download_preflights_outputs_before_readback() {
    let source = resident_download_source();
    let batch_body = source
        .split("fn download_resident_ranges_into(")
        .nth(1)
        .and_then(|tail| tail.split("fn reserve_fused_resident_view_outputs(").next())
        .expect("Fix: WGPU resident download module must expose batch download before fused output preflight.");
    let preflight_pos = batch_body
        .find("reserve_fused_resident_view_outputs(&fused.copies, &fused.views, outputs)?")
        .expect("Fix: WGPU fused resident batch download must preflight caller output slots.");
    let device_queue_pos = batch_body
        .find("let device_queue = backend.current_device_queue();")
        .expect("Fix: WGPU fused resident batch download must stage device readbacks.");
    let materialize_pos = batch_body
        .find("copy_fused_resident_view_into(&fused_outputs, view, output)?")
        .expect("Fix: WGPU fused resident batch download must materialize every fused view.");

    assert!(
        preflight_pos < device_queue_pos && preflight_pos < materialize_pos,
        "Fix: WGPU fused resident batch download must validate fused views and reserve every caller output before any GPU readback or output mutation."
    );

    let preflight = source
        .split("fn reserve_fused_resident_view_outputs(")
        .nth(1)
        .and_then(|tail| tail.split("fn validate_resident_readback_range(").next())
        .expect("Fix: WGPU resident download module must expose fused output preflight before range validation.");
    assert!(
        preflight.contains("views.len() != outputs.len()")
            && preflight.contains("copies.get(view.copy_slot)")
            && preflight.contains("view.byte_offset.checked_add(view.byte_len)")
            && preflight.contains("view_end > copy.byte_len")
            && preflight.contains("try_reserve_vec_to_capacity(*output, view.byte_len)"),
        "Fix: WGPU fused resident output preflight must reject cardinality/view drift and reserve all caller output bytes before materialization."
    );
}

#[test]
fn wgpu_resident_single_ranged_download_validates_bounds_before_readback() {
    let source = resident_download_source();
    let single_body = source
        .split("fn download_resident_range_into(")
        .nth(1)
        .and_then(|tail| tail.split("/// Download several validated resident byte ranges").next())
        .expect("Fix: WGPU resident download module must expose single ranged download before batch ranged download.");
    let validator = source
        .split("fn validate_resident_readback_range(")
        .nth(1)
        .and_then(|tail| tail.split("fn copy_fused_resident_view_into(").next())
        .expect("Fix: WGPU resident download module must expose a shared resident readback range validator.");

    assert!(
        single_body.contains("validate_resident_readback_range(")
            && validator.contains("byte_offset.checked_add(byte_len)")
            && validator.contains("end > allocation_len")
            && validator.contains("requested byte range [{byte_offset}..{end})"),
        "Fix: single WGPU resident ranged download must share the batch path's checked offset/length validation before readback."
    );
}

#[test]
fn wgpu_resident_fused_batch_materialization_reserves_before_clearing_output() {
    let source = resident_download_source();
    let copier = source
        .split("fn copy_fused_resident_view_into(")
        .nth(1)
        .expect("Fix: WGPU resident download module must expose fused output materialization.");
    let reserve_pos = copier
        .find("try_reserve_exact(bytes.len() - output.capacity())")
        .expect("Fix: WGPU fused resident output materialization must reserve caller output storage fallibly.");
    let clear_pos = copier
        .find("output.clear();\n    output.extend_from_slice(bytes);")
        .expect("Fix: WGPU fused resident output materialization must clear and rewrite caller output after successful reservation.");

    assert!(
        reserve_pos < clear_pos,
        "Fix: WGPU fused resident output materialization must reserve before clearing caller-owned output so allocation failure cannot clobber existing bytes."
    );
}


