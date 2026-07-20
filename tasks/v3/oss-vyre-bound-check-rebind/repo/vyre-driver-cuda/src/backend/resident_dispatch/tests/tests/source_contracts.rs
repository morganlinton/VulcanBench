use super::*;

#[test]
fn resident_borrowed_fallback_does_not_allocate_vec_per_fill() {
    let source = super::super::resident_dispatch_production_source();
    assert!(
        source.contains("stage_resident_fill_payload(&mut fill_payload")
            && source.contains("let mut fill_payload = Vec::new();")
            && !source.contains(concat!("vec![value; ", "handle.byte_len]")),
        "Fix: CUDA resident borrowed fallback must stage fills through one reusable Vec, not allocate a fresh Vec per resident clear/fill."
    );
}

#[test]
fn resident_h2d_enqueues_are_single_sourced_without_stealing_stream_order() {
    let source = super::super::resident_dispatch_production_source();
    let production = source
        .split("#[cfg(test)]")
        .next()
        .expect("Fix: resident_dispatch production source must precede tests.");
    assert!(
        production.contains("fn enqueue_resident_h2d_copy")
            && production.contains("fn enqueue_optional_resident_h2d_copy")
            && production.contains("fn enqueue_resident_upload_copies_on_stream")
            && production
                .matches(concat!("crate::backend::copy::", "h2d_async_checked"))
                .count()
                == 1,
        "Fix: resident dispatch parameter uploads, sequence uploads, and per-step parameter uploads must share one local H2D enqueue helper while preserving the caller-owned CUDA stream."
    );
    assert!(
        production.contains("enqueue_resident_upload_copies_on_stream(\n                &upload_copies")
            && production.contains("enqueue_resident_h2d_copy(\n                        params_ptr")
            && production.contains("param_host_ptr,\n                        param_bytes")
            && production.contains("stream.raw(),"),
        "Fix: resident sequence uploads and per-step parameter uploads must use the shared stream-preserving enqueue helpers."
    );
}

#[test]
fn resident_sequence_fill_coalescing_uses_checked_effective_slot_updates() {
    let source = super::super::resident_dispatch_production_source();
    let helper = source
        .split("pub(crate) fn prepare_resident_sequence_fills")
        .nth(1)
        .and_then(|tail| tail.split("pub(crate) struct PreparedStep").next())
        .expect("Fix: resident dispatch helpers must expose prepare_resident_sequence_fills before PreparedStep.");

    assert!(
        helper.contains("effective.get_mut(index)")
            && helper.contains("pointed at stale effective fill slot {index}")
            && !helper.contains("effective[index]"),
        "Fix: duplicate resident sequence fill coalescing must convert stale effective-slot indexes into BackendError instead of panicking."
    );
}

#[test]
fn resident_full_readback_preparation_is_single_sourced() {
    let source = super::super::resident_dispatch_production_source();
    let helper = source
        .split("fn prepare_full_resident_readbacks")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) fn upload_resident_many_sequence_read_ranges_into")
                .next()
        })
        .expect("Fix: resident sequence API must expose full readback preparation before ranged sequence APIs.");
    let readback_reserve = helper
        .find("reserve_smallvec(\n            readbacks")
        .expect("Fix: full resident readback preparation must reserve caller scratch readbacks.");
    let view_cache_reserve = helper
        .find("reserve_smallvec(\n            &mut resident_view_cache")
        .expect("Fix: full resident readback preparation must reserve the resident view cache.");
    let clear = helper.find("readbacks.clear();").expect(
        "Fix: full resident readback preparation must clear reusable scratch before refilling.",
    );

    assert!(
        source.contains("fn prepare_full_resident_readbacks")
            && source
                .matches(concat!("self.", "prepare_full_resident_readbacks(read_handles"))
                .count()
                == 2,
        "Fix: CUDA resident full-handle readback preparation must be shared by read_many and fill_read_many paths."
    );
    assert!(
        readback_reserve < clear && view_cache_reserve < clear,
        "Fix: CUDA resident full-readback preparation must reserve all scratch before clearing reusable readback state."
    );
}

#[test]
fn resident_sequence_output_slot_borrowing_is_single_sourced_and_reuses_slots() {
    let source = super::super::resident_dispatch_production_source();
    let production = source
        .split("#[cfg(test)]")
        .next()
        .expect("Fix: resident_dispatch production source must precede tests.");
    assert!(
        production.contains("fn borrow_resident_sequence_output_slots")
            && production
                .matches("borrow_resident_sequence_output_slots(outputs,")
                .count()
                == 2,
        "Fix: CUDA resident sequence read_many and fill_read_many must share output-slot borrowing."
    );

    let mut outputs = vec![vec![1, 2, 3], Vec::new(), vec![4]];
    let initial_first_capacity = outputs[0].capacity();
    {
        let borrowed = borrow_resident_sequence_output_slots(&mut outputs, 2)
            .expect("Fix: output-slot borrowing should resize existing slots.");
        assert_eq!(borrowed.len(), 2);
    }
    assert_eq!(outputs.len(), 2);
    assert!(
        outputs[0].capacity() >= initial_first_capacity,
        "Fix: resizing borrowed resident output slots must preserve existing slot allocation."
    );
}

#[test]
fn resident_sequence_resolves_views_once_per_sequence() {
    let source = super::super::resident_dispatch_production_source();
    let production = source
        .split("#[cfg(test)]")
        .next()
        .expect("Fix: resident_dispatch production source must precede tests.");

    assert!(
        production.contains("fn resolve_resident_sequence_launch_ptrs")
            && production
                .matches("resolve_resident_sequence_launch_ptrs(step,")
                .count()
                == 1,
        "Fix: CUDA resident sequence launch-pointer validation must be single-sourced."
    );
    assert!(
        production.contains("let mut sequence_view_cache = ResidentViewCache::new();")
            && production.contains("resident sequence view cache")
            && !production.contains("resident sequence fill view cache")
            && !production.contains("resident sequence step view cache")
            && !production.contains("resident sequence readback view cache")
            && !production.contains("struct ClearCopy"),
        "Fix: CUDA resident sequence dispatch must use one sequence-wide resident view cache instead of rebuilding fill, step, and readback caches."
    );
}

#[test]
fn resident_sequence_parameter_cache_growth_is_fallible() {
    let source = super::super::resident_dispatch_production_source();
    let production = source
        .split("#[cfg(test)]")
        .next()
        .expect("Fix: resident_dispatch production source must precede tests.");
    let cache_section = production
        .split("let mut sequence_param_cache")
        .nth(1)
        .expect("Fix: CUDA resident sequence dispatch must keep a per-sequence parameter cache.")
        .split("let mut upload_host_transfers")
        .next()
        .expect(
            "Fix: CUDA resident sequence parameter cache must be reserved before upload staging.",
        );

    assert!(
        production.contains(
            "let mut sequence_param_cache = FxHashMap::<SmallVec<[u32; 8]>, u64>::default();"
        )
            && cache_section.contains("reserve_hash_map(\n            &mut sequence_param_cache")
            && cache_section.contains("prepared_steps.len()")
            && cache_section.contains("\"resident sequence parameter cache\"")
            && production.contains(
                "sequence_param_cache.get(step.prepared.launch.param_words.as_slice())"
            )
            && production.contains("sequence_param_cache.insert(cached_param_words, params_ptr)")
            && !production.contains("sequence_param_cache.iter().find"),
        "Fix: CUDA resident sequence parameter-cache growth must be fallibly reserved to the prepared-step bound and use exact hash lookup instead of rescanning cached launch words."
    );
}

#[test]
fn resident_sequence_error_cleanup_leaks_resources_when_sync_is_unproven() {
    let source = super::super::resident_dispatch_production_source();
    let sequence = source
        .split(
            "pub(crate) fn fill_upload_resident_many_repeated_sequence_read_ranges_borrowed_into",
        )
        .nth(1)
        .expect("Fix: resident sequence fused dispatch function must exist.")
        .split("    }\n}")
        .next()
        .expect("Fix: resident sequence fused dispatch must end inside its module impl.");
    let cleanup = sequence
        .split("if result.is_err()")
        .nth(1)
        .expect("Fix: resident sequence dispatch must handle error cleanup explicitly.")
        .split("self.launch_resources.release_stream(stream);")
        .next()
        .expect("Fix: resident sequence error cleanup must precede stream release.");

    assert!(
        cleanup.contains("match stream.synchronize()")
            && cleanup.contains("Ok(()) => self.telemetry.record_sync_point()")
            && cleanup.contains("Err(error) =>")
            && cleanup.contains("In-flight resident sequence resources will not be recycled.")
            && !cleanup.contains("let _ = stream.synchronize();"),
        "Fix: CUDA resident sequence error cleanup must not ignore failed stream synchronization or record sync telemetry without proof."
    );
    for resource in [
        "stream",
        "resident_use",
        "allocations",
        "host_transfers",
        "upload_host_transfers",
        "readback_host_transfers",
        "timing_events",
    ] {
        assert!(
            cleanup.contains(&format!("std::mem::forget({resource});")),
            "Fix: CUDA resident sequence error cleanup must leak {resource} when stream completion is unproven."
        );
    }
    assert!(
        cleanup.contains("return result;"),
        "Fix: CUDA resident sequence error cleanup must not continue to pooled stream release after leaking in-flight resources."
    );

    let param_upload = sequence
        .split("let param_host_ptr =")
        .nth(1)
        .expect("Fix: resident sequence parameter upload staging must exist.")
        .split("self.telemetry.record_host_to_device_bytes")
        .next()
        .expect("Fix: resident sequence parameter upload must record telemetry after enqueue.");
    let retain_param_staging_pos = param_upload
        .find("host_transfers.push(step_host_transfers);")
        .expect(
            "Fix: resident sequence parameter staging must be retained before async H2D enqueue.",
        );
    let enqueue_param_pos = param_upload
        .find("enqueue_resident_h2d_copy(")
        .expect("Fix: resident sequence parameter upload must enqueue an async H2D copy.");
    assert!(
        retain_param_staging_pos < enqueue_param_pos,
        "Fix: resident sequence parameter host staging must enter outer cleanup ownership before async H2D enqueue."
    );

    let readback = sequence
        .split("readback_host_transfers = Some(HostTransferAllocations::with_capacity")
        .nth(1)
        .expect("Fix: resident sequence readback staging must be owned outside the fallible stream closure.")
        .split("self.telemetry.record_host_to_device_bytes")
        .next()
        .expect("Fix: resident sequence readback staging must precede final telemetry.");
    assert!(
        readback.contains("readback_host_transfers.as_mut()")
            && readback.contains("transfers.push_output(copy.byte_len)?")
            && readback.contains("stream.synchronize()?")
            && readback.contains("transfers.collect_output_range_into"),
        "Fix: resident sequence compact readback staging must remain owned by outer cleanup until stream completion is proven and outputs are collected."
    );
}

#[test]
fn resident_sequence_captures_kernel_device_time_around_launches() {
    let source = super::super::resident_dispatch_production_source();
    let sequence = source
        .split(
            "pub(crate) fn fill_upload_resident_many_repeated_sequence_read_ranges_borrowed_into",
        )
        .nth(1)
        .expect("Fix: resident sequence fused dispatch function must exist.")
        .split("    }\n}")
        .next()
        .expect("Fix: resident sequence fused dispatch must end inside its module impl.");

    // Timing is acquired only when the sequence actually launches kernels, so
    // pure fill/upload/readback sequences never pay for a CUDA event pair, and
    // an exhausted event pool degrades to host-wall-only instead of failing.
    assert!(
        sequence.contains("let has_launch_work = !prefix_step_indices.is_empty()")
            && sequence.contains("|| (repeat_count != 0 && !repeated_step_indices.is_empty());")
            && sequence.contains("if has_launch_work {")
            && sequence.contains("self.launch_resources.acquire_timing_event_pair()"),
        "Fix: CUDA resident sequence must acquire a CUDA timing-event pair, gated on real launch work, to measure kernel device time."
    );

    // The events must bracket exactly the kernel launches: start before the
    // first launch, end after the last, so the measured interval is kernel
    // device time and not host enqueue/readback overhead.
    let start_record = sequence.find("start_event.record(stream.raw())?;").expect(
        "Fix: CUDA resident sequence must record a start timing event before launching kernels.",
    );
    let prefix_launch = sequence
        .find("for &step_index in &prefix_step_indices {")
        .expect("Fix: CUDA resident sequence must launch prefix steps by index.");
    let repeated_launch = sequence
        .find("for &step_index in &repeated_step_indices {")
        .expect("Fix: CUDA resident sequence must launch repeated steps by index.");
    let end_record = sequence.find("end_event.record(stream.raw())?;").expect(
        "Fix: CUDA resident sequence must record an end timing event after launching kernels.",
    );
    assert!(
        start_record < prefix_launch && repeated_launch < end_record,
        "Fix: CUDA resident sequence timing events must bracket the kernel launches (start before the first launch, end after the last)."
    );

    // The kernel interval is recorded through telemetry only on the success
    // path (the closure already synchronized the stream), and a timing-read
    // failure must degrade to a debug log instead of failing a completed scan.
    let success_timing = sequence
        .split("if result.is_ok() {")
        .nth(1)
        .expect("Fix: CUDA resident sequence must record device time on the success path.")
        .split("if result.is_err() {")
        .next()
        .expect("Fix: CUDA resident sequence success-path timing must precede error handling.");
    assert!(
        success_timing
            .contains(".record_timed_dispatch(wall_ns, Some(device_ns), None, None)")
            && success_timing.contains(".elapsed_time_ns(end_event)")
            && success_timing.contains("tracing::debug!")
            && !success_timing.contains(".elapsed_time_ns(end_event)?"),
        "Fix: CUDA resident sequence must record measured kernel device time via telemetry on success and must not turn a timing-read failure into a dispatch error."
    );

    // Events are recycled to the pool on the normal path and forgotten with
    // the other in-flight resources when stream completion is unproven.
    assert!(
        sequence.contains("self.launch_resources.release_timing_event(start_event);")
            && sequence.contains("self.launch_resources.release_timing_event(end_event);"),
        "Fix: CUDA resident sequence must release timing events back to the pool after a successful dispatch."
    );
}

#[test]
fn resident_async_error_cleanup_leaks_resources_when_sync_is_unproven() {
    let source = super::super::resident_dispatch_production_source();
    let dispatch = source
        .split("pub(crate) fn dispatch_resident_async_concrete_with_ptx_key")
        .nth(1)
        .expect("Fix: resident async dispatch function must exist.")
        .split("    }\n}")
        .next()
        .expect("Fix: resident async dispatch must end inside its module impl.");
    assert!(
        dispatch.contains("let mut launch_resources = Some(launch_resources);")
            && dispatch.contains("let mut allocations = Some(allocations);")
            && dispatch.contains("let mut resident_use = Some(resident_use);")
            && dispatch.contains("let mut host_transfers = Some(host_transfers);")
            && dispatch.contains("let enqueue_result = (||"),
        "Fix: CUDA resident async dispatch must retain launch resources, resident use, transient allocations, and pinned host staging in outer cleanup ownership until post-kernel completion is proven."
    );
    assert!(
        dispatch.contains("crate::stream::synchronize_raw_stream(\n                stream_raw,\n                \"cuStreamSynchronize (resident async error cleanup)\",")
            && dispatch.contains("In-flight resident dispatch resources will not be recycled.")
            && dispatch.contains("std::mem::forget(launch_resources);")
            && dispatch.contains("std::mem::forget(allocations);")
            && dispatch.contains("std::mem::forget(resident_use);")
            && dispatch.contains("std::mem::forget(host_transfers);"),
        "Fix: CUDA resident async dispatch must leak in-flight resources when completion is unproven after enqueue errors."
    );
    let cleanup_pos = dispatch
        .find("if let Err(error) = enqueue_result")
        .expect("Fix: resident async dispatch must classify enqueue cleanup errors.");
    let post_kernel_sync_pos = dispatch
        .find("\"cuStreamSynchronize (resident post-kernel)\"")
        .expect("Fix: resident async dispatch must prove completion before synchronous readback.");
    let output_readback_pos = dispatch
        .find("let mut staged_readback_bytes = 0_u64;")
        .expect("Fix: resident async dispatch must keep synchronous output readback after cleanup classification.");
    assert!(
        post_kernel_sync_pos < cleanup_pos && cleanup_pos < output_readback_pos,
        "Fix: resident async dispatch must wrap all fallible enqueue work through the post-kernel fence before releasing resources to synchronous readback."
    );
}

#[test]
fn resident_batch_error_cleanup_leaks_resources_when_sync_is_unproven() {
    let source = super::super::resident_dispatch_production_source();
    let batch = source
        .split("pub(crate) fn dispatch_resident_batch_async_concrete_with_ptx_key")
        .nth(1)
        .expect("Fix: resident batch dispatch function must exist.")
        .split("    }\n}")
        .next()
        .expect("Fix: resident batch dispatch must end inside its module impl.");
    assert!(
        batch.contains("let mut launch_resources = Some(launch_resources);")
            && batch.contains("let mut allocations = Some(allocations);")
            && batch.contains("let mut resident_use = Some(resident_use);")
            && batch.contains("let mut host_transfers = Some(host_transfers);")
            && batch.contains("let pending = (||"),
        "Fix: CUDA resident batch dispatch must retain launch resources, resident use, transient allocations, and pinned host staging in outer cleanup ownership until pending dispatch takes over."
    );
    assert!(
        batch.contains("crate::stream::synchronize_raw_stream(\n                    stream_raw,\n                    \"cuStreamSynchronize (resident batch error cleanup)\",")
            && batch.contains("In-flight resident batch resources will not be recycled.")
            && batch.contains("std::mem::forget(launch_resources);")
            && batch.contains("std::mem::forget(allocations);")
            && batch.contains("std::mem::forget(resident_use);")
            && batch.contains("std::mem::forget(host_transfers);"),
        "Fix: CUDA resident batch dispatch must leak in-flight resources when completion is unproven after enqueue errors."
    );
    let cleanup_pos = batch
        .find("let pending = match pending")
        .expect("Fix: resident batch dispatch must classify pending construction errors.");
    let transfer_pos = batch
        .find("CudaPendingDispatch::new_resident_batch_pending")
        .expect("Fix: resident batch dispatch must eventually transfer ownership to CudaPendingDispatch.");
    assert!(
        transfer_pos < cleanup_pos,
        "Fix: resident batch dispatch must install fail-closed cleanup around all fallible enqueue work before returning pending ownership."
    );
}
