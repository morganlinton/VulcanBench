use super::*;

#[test]

fn compiled_pipeline_dispatch_into_uses_cached_cuda_graph() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");

    let program = add_one_program();
    let inputs = [u32_bytes(&[9, 8, 7, 6, 5, 4, 3, 2])];
    let input_refs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
    let config = DispatchConfig::default();
    backend.reset_telemetry();
    let pipeline = backend
        .compile_native(&program, &config)
        .expect("Fix: CUDA native pipeline compilation must succeed");
    let compile_telemetry = backend.telemetry_snapshot();
    assert_eq!(
        compile_telemetry.sync_points, 1,
        "Fix: CUDA native pipeline compilation must account for static launch-parameter upload synchronization."
    );

    let mut outputs = Vec::with_capacity(1);
    pipeline
        .dispatch_borrowed_into(&input_refs, &config, &mut outputs)
        .expect("Fix: first compiled pipeline dispatch must record and replay a cudaGraph");
    assert_eq!(bytes_u32(&outputs[0]), vec![10, 9, 8, 7, 6, 5, 4, 3]);

    let outer_capacity = outputs.capacity();
    let inner_capacity = outputs[0].capacity();
    pipeline
        .dispatch_borrowed_into(&input_refs, &config, &mut outputs)
        .expect("Fix: second compiled pipeline dispatch must reuse the cached cudaGraph");
    assert_eq!(outputs.capacity(), outer_capacity);
    assert_eq!(outputs[0].capacity(), inner_capacity);
    assert_eq!(bytes_u32(&outputs[0]), vec![10, 9, 8, 7, 6, 5, 4, 3]);
}

#[test]
fn compiled_pipeline_repeated_single_dispatch_uses_exact_materialized_cache_hit() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");

    let program = add_one_program();
    let config = DispatchConfig::default();
    let pipeline = backend
        .compile_native(&program, &config)
        .expect("Fix: CUDA native pipeline compilation must succeed");
    let inputs = [u32_bytes(&[50, 51, 52, 53, 54, 55, 56, 57])];
    let input_refs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
    let mut outputs = Vec::with_capacity(1);

    pipeline
        .dispatch_borrowed_into(&input_refs, &config, &mut outputs)
        .expect("Fix: first compiled single CUDA graph dispatch must materialize outputs.");
    assert_eq!(bytes_u32(&outputs[0]), vec![51, 52, 53, 54, 55, 56, 57, 58]);

    backend.reset_telemetry();
    pipeline
        .dispatch_borrowed_into(&input_refs, &config, &mut outputs)
        .expect("Fix: repeated identical compiled single dispatch must use materialized outputs.");
    assert_eq!(bytes_u32(&outputs[0]), vec![51, 52, 53, 54, 55, 56, 57, 58]);
    let repeated = backend.telemetry_snapshot();
    assert_eq!(
        repeated.cuda_graph_launches, 0,
        "Fix: repeated identical compiled single dispatch must bypass redundant cudaGraph launches."
    );
    assert_eq!(
        repeated.cuda_graph_materialized_cache_hits, 1,
        "Fix: repeated identical compiled single dispatch must report one materialized-cache hit."
    );

    let changed = [u32_bytes(&[70, 71, 72, 73, 74, 75, 76, 77])];
    let changed_refs: Vec<&[u8]> = changed.iter().map(Vec::as_slice).collect();
    backend.reset_telemetry();
    pipeline
        .dispatch_borrowed_into(&changed_refs, &config, &mut outputs)
        .expect("Fix: changed single-dispatch input bytes must bypass materialized cache.");
    assert_eq!(bytes_u32(&outputs[0]), vec![71, 72, 73, 74, 75, 76, 77, 78]);
    let changed_telemetry = backend.telemetry_snapshot();
    assert_eq!(
        changed_telemetry.cuda_graph_launches, 1,
        "Fix: changed compiled single-dispatch input bytes must launch exactly one cudaGraph replay."
    );
    assert_eq!(
        changed_telemetry.cuda_graph_materialized_cache_hits, 0,
        "Fix: changed compiled single-dispatch input bytes must not count as a materialized-cache hit."
    );
}

#[test]
fn compiled_pipeline_repeated_timed_single_dispatch_reports_materialized_zero_device_work() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");

    let program = add_one_program();
    let config = DispatchConfig::default();
    let pipeline = backend
        .compile_native(&program, &config)
        .expect("Fix: CUDA native pipeline compilation must succeed");
    let inputs = [u32_bytes(&[80, 81, 82, 83, 84, 85, 86, 87])];
    let input_refs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();

    let first = pipeline
        .dispatch_borrowed_timed(&input_refs, &config)
        .expect("Fix: first timed compiled single dispatch must execute and materialize outputs.");
    assert_eq!(
        bytes_u32(&first.outputs[0]),
        vec![81, 82, 83, 84, 85, 86, 87, 88]
    );
    assert!(
        first.device_ns.unwrap_or(0) > 0,
        "Fix: first timed compiled graph dispatch must report real device work before materialized hits exist."
    );

    backend.reset_telemetry();
    let repeated = pipeline
        .dispatch_borrowed_timed(&input_refs, &config)
        .expect("Fix: repeated timed compiled single dispatch must use materialized outputs.");
    assert_eq!(
        bytes_u32(&repeated.outputs[0]),
        vec![81, 82, 83, 84, 85, 86, 87, 88]
    );
    assert_eq!(
        repeated.device_ns,
        Some(0),
        "Fix: timed materialized-cache hits must report zero device work instead of replaying a graph for timing."
    );
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.cuda_graph_launches, 0,
        "Fix: repeated timed compiled single dispatch must not launch cudaGraph work."
    );
    assert_eq!(
        telemetry.cuda_graph_materialized_cache_hits, 1,
        "Fix: repeated timed compiled single dispatch must report one materialized-cache hit."
    );
}

#[test]
fn compiled_pipeline_batched_cuda_graph_replay_matches_direct_dispatch() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");

    let program = add_one_program();
    let config = DispatchConfig::default();
    let pipeline = backend
        .compile_native(&program, &config)
        .expect("Fix: CUDA native pipeline compilation must succeed");
    backend.reset_telemetry();
    let inputs = [
        u32_bytes(&[0, 1, 2, 3, 4, 5, 6, 7]),
        u32_bytes(&[10, 11, 12, 13, 14, 15, 16, 17]),
        u32_bytes(&[20, 21, 22, 23, 24, 25, 26, 27]),
        u32_bytes(&[30, 31, 32, 33, 34, 35, 36, 37]),
    ];
    let batch0 = [inputs[0].as_slice()];
    let batch1 = [inputs[1].as_slice()];
    let batch2 = [inputs[2].as_slice()];
    let batch3 = [inputs[3].as_slice()];
    let batches: [&[&[u8]]; 4] = [&batch0, &batch1, &batch2, &batch3];
    let mut outputs = Vec::with_capacity(batches.len());

    pipeline
        .dispatch_borrowed_batched_into(&batches, &config, &mut outputs)
        .expect("Fix: compiled batched CUDA dispatch must replay same-shape graph batches");

    assert_eq!(outputs.len(), batches.len());
    for (index, output) in outputs.iter().enumerate() {
        let expected: Vec<u32> = (0..8)
            .map(|offset| (index as u32) * 10 + offset + 1)
            .collect();
        assert_eq!(
            bytes_u32(&output[0]),
            expected,
            "Fix: batched cudaGraph replay lane {index} must match direct add-one semantics"
        );
    }
    let telemetry = backend.telemetry_snapshot();
    assert!(
        telemetry.cuda_graph_batched_replay_chunks >= 1,
        "Fix: same-shape compiled batch replay must report batched cudaGraph chunks, not hide behind per-item replay telemetry."
    );
    assert!(
        telemetry.cuda_graph_batched_replay_lanes >= batches.len() as u64,
        "Fix: same-shape compiled batch replay must report every graph lane launched."
    );
}

#[test]
fn compiled_pipeline_repeated_batched_cuda_graph_uses_exact_materialized_cache_hits() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");

    let program = add_one_program();
    let config = DispatchConfig::default();
    let pipeline = backend
        .compile_native(&program, &config)
        .expect("Fix: CUDA native pipeline compilation must succeed");
    let inputs = [
        u32_bytes(&[100, 101, 102, 103, 104, 105, 106, 107]),
        u32_bytes(&[200, 201, 202, 203, 204, 205, 206, 207]),
        u32_bytes(&[300, 301, 302, 303, 304, 305, 306, 307]),
        u32_bytes(&[400, 401, 402, 403, 404, 405, 406, 407]),
    ];
    let batch0 = [inputs[0].as_slice()];
    let batch1 = [inputs[1].as_slice()];
    let batch2 = [inputs[2].as_slice()];
    let batch3 = [inputs[3].as_slice()];
    let batches: [&[&[u8]]; 4] = [&batch0, &batch1, &batch2, &batch3];
    let mut outputs = Vec::with_capacity(batches.len());

    pipeline
        .dispatch_borrowed_batched_into(&batches, &config, &mut outputs)
        .expect("Fix: first compiled batched CUDA graph replay must materialize lane outputs.");

    backend.reset_telemetry();
    pipeline
        .dispatch_borrowed_batched_into(&batches, &config, &mut outputs)
        .expect(
            "Fix: repeated identical compiled batch must reuse materialized CUDA graph outputs.",
        );

    for (index, output) in outputs.iter().enumerate() {
        let base = ((index as u32) + 1) * 100;
        let expected: Vec<u32> = (0..8).map(|offset| base + offset + 1).collect();
        assert_eq!(
            bytes_u32(&output[0]),
            expected,
            "Fix: materialized-cache batch lane {index} must return exact cached add-one output."
        );
    }
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.cuda_graph_launches, 0,
        "Fix: repeated identical compiled batches must prefer exact materialized cached lanes and avoid all redundant graph launches."
    );
    assert_eq!(
        telemetry.cuda_graph_materialized_cache_hits,
        batches.len() as u64,
        "Fix: every repeated batch lane must be counted as a materialized CUDA graph cache hit."
    );
    assert_eq!(
        telemetry.cuda_graph_batched_replay_lanes, 0,
        "Fix: all-hit materialized batches must not report launched batched replay lanes."
    );
}

#[test]
fn compiled_pipeline_mixed_batched_materialized_cache_launches_only_misses() {
    let backend = CudaBackend::acquire()
        .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host.");

    let program = add_one_program();
    let config = DispatchConfig::default();
    let pipeline = backend
        .compile_native(&program, &config)
        .expect("Fix: CUDA native pipeline compilation must succeed");
    let cached_inputs = [
        u32_bytes(&[10, 11, 12, 13, 14, 15, 16, 17]),
        u32_bytes(&[20, 21, 22, 23, 24, 25, 26, 27]),
        u32_bytes(&[30, 31, 32, 33, 34, 35, 36, 37]),
        u32_bytes(&[40, 41, 42, 43, 44, 45, 46, 47]),
    ];
    let cached0 = [cached_inputs[0].as_slice()];
    let cached1 = [cached_inputs[1].as_slice()];
    let cached2 = [cached_inputs[2].as_slice()];
    let cached3 = [cached_inputs[3].as_slice()];
    let cached_batches: [&[&[u8]]; 4] = [&cached0, &cached1, &cached2, &cached3];
    let mut outputs = Vec::with_capacity(cached_batches.len());

    pipeline
        .dispatch_borrowed_batched_into(&cached_batches, &config, &mut outputs)
        .expect("Fix: first compiled batched CUDA graph replay must materialize cached lanes.");

    let changed = u32_bytes(&[90, 91, 92, 93, 94, 95, 96, 97]);
    let changed1 = [changed.as_slice()];
    let mixed_batches: [&[&[u8]]; 4] = [&cached0, &changed1, &cached2, &cached3];
    backend.reset_telemetry();
    pipeline
        .dispatch_borrowed_batched_into(&mixed_batches, &config, &mut outputs)
        .expect("Fix: mixed materialized/miss batch must replay only cache misses.");

    let expected = [
        vec![11, 12, 13, 14, 15, 16, 17, 18],
        vec![91, 92, 93, 94, 95, 96, 97, 98],
        vec![31, 32, 33, 34, 35, 36, 37, 38],
        vec![41, 42, 43, 44, 45, 46, 47, 48],
    ];
    for (index, output) in outputs.iter().enumerate() {
        assert_eq!(
            bytes_u32(&output[0]),
            expected[index],
            "Fix: mixed CUDA materialized batch lane {index} must return exact add-one output."
        );
    }
    let telemetry = backend.telemetry_snapshot();
    assert_eq!(
        telemetry.cuda_graph_launches, 1,
        "Fix: mixed materialized/miss compiled batches must launch only the one cache-miss lane."
    );
    assert_eq!(
        telemetry.cuda_graph_materialized_cache_hits, 3,
        "Fix: mixed materialized/miss compiled batches must count the three exact cache-hit lanes."
    );
    assert_eq!(
        telemetry.cuda_graph_batched_replay_lanes, 1,
        "Fix: mixed materialized/miss compiled batches must report only launched graph lanes."
    );
}

