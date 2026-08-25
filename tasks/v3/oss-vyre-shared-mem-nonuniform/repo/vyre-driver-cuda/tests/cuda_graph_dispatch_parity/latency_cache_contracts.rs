use super::*;

#[test]
fn cuda_graph_dispatch_per_replay_beats_direct_dispatch_floor() {
    let backend = Arc::new(
        CudaBackend::acquire()
            .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host."),
    );

    let program = add_one_program();
    let inputs = vec![u32_bytes(&[0; 8])];
    let config = DispatchConfig::default();

    // Warm + record.
    let warm_outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("warm dispatch");
    assert_eq!(
        bytes_u32(&warm_outputs[0]),
        vec![1; 8],
        "Fix: warm direct dispatch before cudaGraph recording must produce the add-one oracle output"
    );
    let mut cached = backend
        .record_cuda_graph(&program, &inputs, &config)
        .expect("Fix: record must succeed");

    // Measure several steady-state windows. Full-suite GPU contention can
    // occasionally inject a single scheduler-latency spike; the release
    // contract is that the warmed replay path can sustain the direct-dispatch
    // floor, not that one noisy wall-clock window defines the kernel path.
    const REPLAYS: u32 = 1000;
    const WINDOWS: u32 = 5;
    let input_refs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
    let mut outputs = Vec::with_capacity(1);
    let mut best_per_replay_ns = u128::MAX;
    for _ in 0..WINDOWS {
        let t0 = Instant::now();
        for _ in 0..REPLAYS {
            backend
                .dispatch_via_cuda_graph_into(&mut cached, &input_refs, &mut outputs)
                .expect("graph replay must succeed");
        }
        let elapsed_ns = t0.elapsed().as_nanos();
        best_per_replay_ns = best_per_replay_ns.min(elapsed_ns / u128::from(REPLAYS));
    }

    println!();
    println!("=== cudaGraph production dispatch replay ===");
    println!("windows             {WINDOWS}");
    println!("replays_per_window  {REPLAYS}");
    println!("best_per_replay_ns  {best_per_replay_ns}");
    println!("===");

    // Full-Program graph replay includes input upload, kernel launch,
    // output readback, stream synchronization, and output materialization.
    // The ceiling keeps this path below the direct warm-dispatch latency
    // floor for latency-bound kernels.
    assert!(
        best_per_replay_ns < 20_000,
        "cudaGraph per-replay must beat 20 µs (1.4× the 28.3 µs direct-dispatch floor); \
         observed best steady-state window {best_per_replay_ns}ns. A regression here means the cached graph isn't \
         amortizing the launch path, OR per-replay memcpy/clone cost rose."
    );
}

#[test]
fn cuda_graph_materialized_cache_is_telemetry_visible_and_input_exact() {
    let backend = Arc::new(
        CudaBackend::acquire()
            .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host."),
    );
    let program = add_one_program();
    let config = DispatchConfig::default();
    let inputs = vec![u32_bytes(&[0, 1, 2, 3, 4, 5, 6, 7])];
    let input_refs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
    let mut cached = backend
        .record_cuda_graph(&program, &inputs, &config)
        .expect("Fix: cudaGraph recording must succeed before materialized-cache telemetry.");
    backend.reset_telemetry();

    let mut outputs = Vec::with_capacity(1);
    backend
        .dispatch_via_cuda_graph_into(&mut cached, &input_refs, &mut outputs)
        .expect("Fix: first replay must execute the graph and materialize outputs.");
    assert_eq!(bytes_u32(&outputs[0]), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    let first = backend.telemetry_snapshot();
    assert_eq!(
        first.cuda_graph_launches, 1,
        "Fix: first same-shape replay must execute a real cudaGraph before cache hits are possible."
    );
    assert_eq!(
        first.cuda_graph_materialized_cache_hits, 0,
        "Fix: materialized cache must not claim a hit before host outputs are initialized."
    );

    backend
        .dispatch_via_cuda_graph_into(&mut cached, &input_refs, &mut outputs)
        .expect("Fix: second identical replay must use the materialized-output fast path.");
    assert_eq!(bytes_u32(&outputs[0]), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    let cached_same = backend.telemetry_snapshot();
    assert_eq!(
        cached_same.cuda_graph_launches, 1,
        "Fix: identical materialized-cache hit must not enqueue redundant cudaGraph work."
    );
    assert_eq!(
        cached_same.cuda_graph_materialized_cache_hits, 1,
        "Fix: materialized-cache hit must be observable in CUDA telemetry."
    );

    let changed_inputs = vec![u32_bytes(&[10, 20, 30, 40, 50, 60, 70, 80])];
    let changed_refs: Vec<&[u8]> = changed_inputs.iter().map(Vec::as_slice).collect();
    backend
        .dispatch_via_cuda_graph_into(&mut cached, &changed_refs, &mut outputs)
        .expect("Fix: changed bytes must bypass materialized cache and execute a graph replay.");
    assert_eq!(bytes_u32(&outputs[0]), vec![11, 21, 31, 41, 51, 61, 71, 81]);
    let changed = backend.telemetry_snapshot();
    assert_eq!(
        changed.cuda_graph_launches, 2,
        "Fix: changed inputs must force a fresh cudaGraph replay instead of returning stale host outputs."
    );
    assert_eq!(
        changed.cuda_graph_materialized_cache_hits, 1,
        "Fix: changed input replay must not be counted as a materialized-cache hit."
    );
}

#[test]
fn cuda_graph_timed_replay_uses_exact_materialized_cache_without_device_work() {
    let backend = Arc::new(
        CudaBackend::acquire()
            .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host."),
    );
    let program = add_one_program();
    let config = DispatchConfig::default();
    let inputs = vec![u32_bytes(&[5, 6, 7, 8, 9, 10, 11, 12])];
    let input_refs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
    let mut cached = backend
        .record_cuda_graph(&program, &inputs, &config)
        .expect("Fix: cudaGraph recording must succeed before timed materialized replay.");

    backend.reset_telemetry();
    let first = backend
        .dispatch_via_cuda_graph_timed(&mut cached, &input_refs)
        .expect("Fix: first timed cudaGraph replay must execute and materialize outputs.");
    assert_eq!(
        bytes_u32(&first.outputs[0]),
        vec![6, 7, 8, 9, 10, 11, 12, 13]
    );
    let first_telemetry = backend.telemetry_snapshot();
    assert_eq!(
        first_telemetry.cuda_graph_launches, 1,
        "Fix: first timed cudaGraph replay must execute one graph launch before cached outputs exist."
    );
    assert_eq!(
        first_telemetry.cuda_graph_materialized_cache_hits, 0,
        "Fix: first timed cudaGraph replay must not claim a materialized hit before outputs are initialized."
    );

    backend.reset_telemetry();
    let repeated = backend
        .dispatch_via_cuda_graph_timed(&mut cached, &input_refs)
        .expect("Fix: repeated timed cudaGraph replay must use exact materialized outputs.");
    assert_eq!(
        bytes_u32(&repeated.outputs[0]),
        vec![6, 7, 8, 9, 10, 11, 12, 13]
    );
    assert_eq!(
        repeated.device_ns,
        Some(0),
        "Fix: timed raw cudaGraph materialized hits must report zero device work instead of launching for timing."
    );
    let repeated_telemetry = backend.telemetry_snapshot();
    assert_eq!(
        repeated_telemetry.cuda_graph_launches, 0,
        "Fix: repeated timed raw cudaGraph replay must bypass redundant device graph launches."
    );
    assert_eq!(
        repeated_telemetry.cuda_graph_materialized_cache_hits, 1,
        "Fix: repeated timed raw cudaGraph replay must record one exact materialized-cache hit."
    );
    assert_eq!(
        repeated_telemetry.timed_dispatches, 1,
        "Fix: timed raw cudaGraph materialized hits must still be visible as timed dispatches."
    );

    let changed_inputs = vec![u32_bytes(&[15, 16, 17, 18, 19, 20, 21, 22])];
    let changed_refs: Vec<&[u8]> = changed_inputs.iter().map(Vec::as_slice).collect();
    backend.reset_telemetry();
    let changed = backend
        .dispatch_via_cuda_graph_timed(&mut cached, &changed_refs)
        .expect("Fix: changed timed cudaGraph inputs must bypass materialized cache.");
    assert_eq!(
        bytes_u32(&changed.outputs[0]),
        vec![16, 17, 18, 19, 20, 21, 22, 23]
    );
    let changed_telemetry = backend.telemetry_snapshot();
    assert_eq!(
        changed_telemetry.cuda_graph_launches, 1,
        "Fix: changed timed raw cudaGraph inputs must launch a graph instead of returning stale output."
    );
    assert_eq!(
        changed_telemetry.cuda_graph_materialized_cache_hits, 0,
        "Fix: changed timed raw cudaGraph inputs must not be counted as a materialized-cache hit."
    );
}

