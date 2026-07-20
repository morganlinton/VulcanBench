use super::*;

#[test]
fn cuda_compiled_pipeline_identity_uses_shared_domain_separated_contract() {
    for seed in 0_u32..2048 {
        let ptx_key = generated_pipeline_identity_key(seed, 0x5054_5820);
        let module_key = generated_pipeline_identity_key(seed, 0x4D4F_4420);
        let launch = generated_pipeline_identity_launch(seed);

        let key = cuda_compiled_pipeline_identity_key(&ptx_key, &module_key, &launch)
            .expect("Fix: generated CUDA compiled pipeline key must fit");
        let changed_ptx = cuda_compiled_pipeline_identity_key(
            &generated_pipeline_identity_key(seed ^ 1, 0x5054_5820),
            &module_key,
            &launch,
        )
        .expect("Fix: generated CUDA compiled pipeline PTX variant must fit");
        let changed_module = cuda_compiled_pipeline_identity_key(
            &ptx_key,
            &generated_pipeline_identity_key(seed ^ 1, 0x4D4F_4420),
            &launch,
        )
        .expect("Fix: generated CUDA compiled pipeline module variant must fit");
        let mut changed_launch = launch.clone();
        changed_launch.grid[0] = changed_launch.grid[0].wrapping_add(1);
        let changed_launch_key =
            cuda_compiled_pipeline_identity_key(&ptx_key, &module_key, &changed_launch)
                .expect("Fix: generated CUDA compiled pipeline launch variant must fit");

        assert_ne!(key, changed_ptx);
        assert_ne!(key, changed_module);
        assert_ne!(key, changed_launch_key);
    }
}

#[test]
fn cuda_compiled_pipeline_source_does_not_fork_blake3_tuple_hashing() {
    let source = include_str!("../../pipeline.rs");
    assert!(
        source.contains("domain_separated_exact_input_key")
            && source.contains("cuda_compiled_pipeline_identity_key")
            && !source.contains(&["blake", "3::Hasher::new()"].concat()),
        "Fix: CUDA compiled pipeline identity must use the shared domain-separated exact-input key instead of local BLAKE3 tuple hashing."
    );
}

#[test]
fn cuda_pipeline_dynamic_dispatch_reuses_existing_output_slots() {
    let mut outputs = vec![Vec::with_capacity(8), Vec::with_capacity(4)];
    let outputs_addr = outputs.as_ptr() as usize;
    let first_slot_addr = outputs[0].as_ptr() as usize;
    let second_slot_addr = outputs[1].as_ptr() as usize;

    replace_output_buffers_preserving_slots(vec![vec![1, 2, 3], vec![4]], &mut outputs);

    assert_eq!(outputs, vec![vec![1, 2, 3], vec![4]]);
    assert_eq!(outputs.as_ptr() as usize, outputs_addr);
    assert_eq!(outputs[0].as_ptr() as usize, first_slot_addr);
    assert_eq!(outputs[1].as_ptr() as usize, second_slot_addr);
}

#[test]
fn cuda_graph_lane_planner_scales_past_legacy_four_lane_cap() {
    let caps = blackwell_sm120_caps(32 * 1024 * 1024 * 1024);
    let plan = single_input_output_plan(1024);
    let input = vec![7_u8; 1024];
    let row = [input.as_slice()];
    let batches: Vec<&[&[u8]]> = vec![row.as_slice(); 64];

    let lanes = cuda_graph_lane_count_for_batch(&caps, &plan, &batches)
        .expect("Fix: graph replay lane planning should fit");

    assert!(lanes > 4);
    assert_eq!(lanes, 22);
}

#[test]
fn cuda_graph_lane_planner_caps_large_graphs_by_vram_budget() {
    let caps = blackwell_sm120_caps(512 * 1024 * 1024);
    let plan = single_input_output_plan(64 * 1024 * 1024);
    let input = vec![1_u8; 64 * 1024 * 1024];
    let row = [input.as_slice()];
    let batches: Vec<&[&[u8]]> = vec![row.as_slice(); 64];

    let lanes = cuda_graph_lane_count_for_batch(&caps, &plan, &batches)
        .expect("Fix: graph replay lane planning should fit");

    assert_eq!(lanes, 1);
}

#[test]
fn cuda_graph_replay_is_release_default_not_opt_in_debug_path() {
    let source = include_str!("../../instrumentation.rs");
    let pipeline_source = include_str!("../../pipeline.rs");

    assert!(
        source.contains("VYRE_CUDA_GRAPH_REPLAY")
            && source.contains("cached_enabled_default_true")
            && source.contains("CUDA_GRAPH_REPLAY_DISABLED"),
        "Fix: CUDA graph replay must be enabled by default with only an explicit debug disable."
    );
    assert!(
        pipeline_source.contains("crate::instrumentation::cuda_graph_replay_enabled()")
            && !pipeline_source.contains("std::env::var(\"VYRE_CUDA_GRAPH_REPLAY\")")
            && !pipeline_source.contains("var_os(\"VYRE_CUDA_GRAPH_REPLAY\")"),
        "Fix: CUDA graph replay must not be opt-in on the release path."
    );
}

#[test]
fn static_launch_param_upload_sync_is_telemetry_visible() {
    let source = include_str!("../static_params.rs");
    assert!(
        source.contains("enum StaticParamUploadFailure")
            && source.contains("Completed(BackendError)")
            && source.contains("CompletionUnproven(BackendError)"),
        "Fix: CUDA static launch parameter upload must distinguish completed cleanup failures from unproven in-flight failures."
    );
    let upload = source
        .split("pub(crate) fn upload_static_launch_params")
        .nth(1)
        .expect("Fix: CUDA static launch parameter upload helper must exist.");
    assert!(
        upload.contains("backend.telemetry.record_sync_point();"),
        "Fix: CUDA compiled-pipeline static parameter upload must record its stream synchronization in telemetry."
    );
    assert!(
        upload.contains("if let Err(error) = enqueue_result")
            && upload.contains("match stream.synchronize()")
            && upload.contains("In-flight static parameter upload resources will not be recycled.")
            && upload.contains("std::mem::forget(stream);")
            && upload.contains("StaticParamUploadFailure::CompletionUnproven(error)"),
        "Fix: CUDA compiled-pipeline static parameter upload must not recycle its stream after enqueue errors unless completion is proven."
    );
    assert!(
        upload.contains("Err(StaticParamUploadFailure::Completed(err)) =>")
            && upload.contains("backend.transient_pool.release(allocation);")
            && upload.contains("Err(StaticParamUploadFailure::CompletionUnproven(err)) =>")
            && upload.contains("let _unreleased_allocation = allocation;")
            && upload.contains("std::mem::forget(host_transfers);"),
        "Fix: CUDA compiled-pipeline static parameter upload must not recycle device or host staging allocations when upload completion is unproven."
    );
    let unproven_cleanup = upload
        .split("Err(StaticParamUploadFailure::CompletionUnproven(err)) =>")
        .nth(1)
        .expect("Fix: static parameter upload must have unproven-completion cleanup.")
        .split("backend.telemetry.record_host_to_device_bytes")
        .next()
        .expect("Fix: unproven static upload cleanup must precede success telemetry.");
    assert!(
        !unproven_cleanup.contains("transient_pool.release"),
        "Fix: CUDA static parameter upload must not return unproven in-flight device memory to the transient pool."
    );
    assert!(
        upload.contains("if let Err(error) = stream.synchronize()")
            && upload.contains("backend.telemetry.record_sync_point();")
            && upload.contains("backend.launch_resources.release_stream(stream);"),
        "Fix: CUDA compiled-pipeline static parameter upload must check synchronization before telemetry or stream release."
    );
    let sync_pos = upload
        .find("if let Err(error) = stream.synchronize()")
        .expect("Fix: static parameter upload must synchronize before releasing the stream.");
    let telemetry_pos = upload
        .rfind("backend.telemetry.record_sync_point();")
        .expect("Fix: static parameter upload must record sync telemetry after success.");
    let release_pos = upload
        .rfind("backend.launch_resources.release_stream(stream);")
        .expect("Fix: static parameter upload must release the stream after successful synchronization.");
    assert!(
        sync_pos < telemetry_pos && telemetry_pos < release_pos,
        "Fix: CUDA compiled-pipeline static parameter upload must prove completion before telemetry or pooled stream release."
    );
}

#[test]
fn cuda_graph_shape_bytes_overflow_fails_loudly_without_saturating_arithmetic() {
    assert_eq!(add_shape_bytes(usize::MAX - 1, 1).unwrap(), usize::MAX);
    let overflow = add_shape_bytes(usize::MAX - 1, 2);
    assert!(
        matches!(overflow, Err(vyre_driver::BackendError::InvalidProgram { .. })),
        "Fix: CUDA graph replay shape byte overflow must return a typed error instead of capping or panicking."
    );

    let source = include_str!("../../pipeline.rs");
    assert!(
        !source.contains(concat!(".saturating_add", "(CUDA_GRAPH_REPLAY_SMS_PER_LANE"))
            && !source.contains(concat!("bytes = bytes", ".saturating_add")),
        "Fix: CUDA graph lane planning must use exact arithmetic with an explicit overflow cap, not generic saturating arithmetic."
    );
    assert!(
        !source.contains("unwrap_or(usize::MAX)"),
        "Fix: CUDA graph replay shape byte overflow must not silently cap to usize::MAX."
    );
}

#[test]
fn compiled_cuda_graph_batched_replay_uses_checked_batch_lane_and_output_slots() {
    let source = include_str!("../compiled_dispatch.rs");

    assert!(
        source.contains("fn compiled_graph_batch_inputs")
            && source.contains("fn compiled_graph_output_mut")
            && source.contains("fn compiled_graph_lane")
            && source.contains("fn compiled_graph_lane_mut")
            && source.contains(".get(batch_index)")
            && source.contains(".get_mut(batch_index)")
            && source.contains("miss_batches\n                .first()\n                .copied()")
            && source.contains(".get(lane)")
            && source.contains(".get_mut(lane)"),
        "Fix: compiled CUDA graph batched replay must use typed accessors for batch inputs, output slots, and lane slots."
    );
    assert!(
        !source.contains("batches[batch_index]")
            && !source.contains("outputs[batch_index]")
            && !source.contains("batches[launched_batch.batch_index]")
            && !source.contains("outputs[launched_batch.batch_index]")
            && !source.contains("miss_batches[0]")
            && !source.contains(concat!("lanes", "[lane]"))
            && !source.contains("lanes[launched_batch.lane]"),
        "Fix: compiled CUDA graph batched replay must return BackendError for stale replay indexes instead of panicking on direct indexing."
    );
    assert!(
        source.contains("fn finish_and_return_cuda_graph_lanes_after_error")
            && source.contains("fn return_cached_graph_lanes_after_error")
            && source.contains("fn finish_cuda_graph_lane_replay_discarding_outputs")
            && source.contains("return self.finish_and_return_cuda_graph_lanes_after_error(")
            && source.contains("return self.return_cached_graph_lanes_after_error(lanes, error)")
            && source.matches("std::mem::forget(lanes);").count() >= 2
            && source.matches("std::mem::forget(cached);").count() >= 2
            && !source.contains("finish_cuda_graph_indexed_lane_replays(&mut lanes, launched, outputs)?")
            && !source.contains("compiled_graph_output_mut(\n                            outputs,\n                            batch_index,\n                            \"materialized cache probe\",\n                        )?"),
        "Fix: compiled CUDA graph batched replay must finish launched lanes and either return reusable cached graph lanes or leak unproven-completion lanes instead of bypassing cleanup with direct `?` exits."
    );
    let timed_single = source
        .split("fn dispatch_borrowed_timed(")
        .nth(1)
        .expect("Fix: timed compiled-pipeline dispatch must remain present.")
        .split("fn dispatch_borrowed_into(")
        .next()
        .expect("Fix: timed compiled-pipeline dispatch must precede untimed dispatch.");
    let untimed_single = source
        .split("fn dispatch_borrowed_into(")
        .nth(1)
        .expect("Fix: untimed compiled-pipeline dispatch must remain present.")
        .split("fn dispatch_borrowed_batched(")
        .next()
        .expect("Fix: untimed compiled-pipeline dispatch must precede batched dispatch.");
    assert!(
        timed_single.contains("self.return_cached_graph(cached)?")
            && timed_single.contains("std::mem::forget(cached);")
            && untimed_single.contains("self.return_cached_graph(cached)?")
            && untimed_single.contains("std::mem::forget(cached);"),
        "Fix: single CUDA graph replay must return cached graphs only after successful replay and leak them when replay completion is unproven."
    );
    assert!(
        timed_single.contains("let input_key = materialized_input_key(inputs)?;")
            && timed_single.contains("materialized_output_cache_hit_with_key_into(inputs, &input_key")
            && timed_single.contains("take_cached_graph_with_replay_state(inputs, &input_key)")
            && timed_single.contains("Some(selection) => (selection.graph, selection.input_state)")
            && timed_single.contains("prepare_cuda_graph_replay_input_state_with_key")
            && timed_single.contains("&cached")
            && timed_single.contains("input_key")
            && timed_single.contains("dispatch_via_cuda_graph_timed_with_input_state_into")
            && timed_single.contains("remember_materialized_output_cache_with_key(inputs, input_key"),
        "Fix: timed single CUDA graph replay must reuse one exact-input key and one validated cached-graph input state across pipeline cache, graph selection, raw graph replay, and materialized-cache storage."
    );
    assert!(
        untimed_single.contains("let input_key = materialized_input_key(inputs)?;")
            && untimed_single.contains("materialized_output_cache_hit_with_key_into(inputs, &input_key")
            && untimed_single.contains("take_cached_graph_with_replay_state(inputs, &input_key)")
            && untimed_single.contains("Some(selection) => (selection.graph, selection.input_state)")
            && untimed_single.contains("prepare_cuda_graph_replay_input_state_with_key")
            && untimed_single.contains("&cached")
            && untimed_single.contains("input_key")
            && untimed_single.contains("dispatch_via_cuda_graph_with_input_state_into")
            && untimed_single.contains("remember_materialized_output_cache_with_key(inputs, input_key"),
        "Fix: untimed single CUDA graph replay must reuse one exact-input key and one validated cached-graph input state across pipeline cache, graph selection, raw graph replay, and materialized-cache storage."
    );
    assert!(
        source.contains("fn materialized_output_cache_hit_with_key_into")
            && source.contains("cache.snapshot_with_key(inputs, input_key)")
            && source.contains("fn take_cached_graph_with_key")
            && source.contains("fn take_cached_graph_with_replay_state")
            && source.contains("struct CachedGraphReplaySelection")
            && source.contains("first_shape_match = Some((index, input_state));")
            && source.contains("graph: graphs.swap_remove(index)")
            && source.contains("materialized_output_cache_matches_with_input_state(inputs, &input_state)")
            && source.contains("fn remember_materialized_output_cache_with_key")
            && !source.contains("fn take_cached_graph(")
            && !source.contains("fn remember_materialized_output_cache("),
        "Fix: compiled CUDA graph single replay helpers must consume precomputed input keys and carry validated replay input state out of graph-cache selection instead of recomputing it."
    );
    let batched_replay = source
        .split("fn dispatch_borrowed_batched_via_cuda_graph_lanes")
        .nth(1)
        .expect("Fix: batched compiled-pipeline graph replay must remain present.")
        .split("fn materialized_output_batch_cache_partition_into")
        .next()
        .expect("Fix: batched compiled-pipeline graph replay must precede materialized cache partition.");
    assert!(
        batched_replay.contains("let first_miss")
            && batched_replay.contains("miss_entries")
            && batched_replay.contains("take_cached_graph_with_key(")
            && batched_replay.contains("first_miss_batch")
            && batched_replay.contains("&first_miss.input_key")
            && !batched_replay.contains("take_cached_graph(first_miss_batch)"),
        "Fix: batched CUDA graph lane seeding must reuse the partitioned miss input key for graph-cache selection instead of hashing the first miss again."
    );
    let finish_helper = source
        .split("fn finish_cuda_graph_indexed_lane_replays")
        .nth(1)
        .expect("Fix: compiled CUDA graph replay must expose indexed lane finishing.")
        .split("fn finish_and_return_cuda_graph_lanes_after_error")
        .next()
        .expect("Fix: compiled CUDA graph lane finishing must precede error-return helper.");
    assert!(
        finish_helper.contains("finish_cuda_graph_lane_replay_discarding_outputs")
            && finish_helper.contains("lane.output_host_bufs.len()")
            && finish_helper.contains("\"discarded cuda graph lane output\"")
            && finish_helper.contains("finish_cuda_graph_replay_into(lane, replay_stats, &mut discard_outputs)"),
        "Fix: launched CUDA graph lanes must be fenced through finish_cuda_graph_replay_into even when caller output-slot lookup fails."
    );
}

