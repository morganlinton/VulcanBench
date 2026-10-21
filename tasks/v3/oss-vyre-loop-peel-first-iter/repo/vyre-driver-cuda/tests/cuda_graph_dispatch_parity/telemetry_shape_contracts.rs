use super::*;

#[test]
fn cuda_graph_recording_accounts_raw_device_allocations() {
    let backend = Arc::new(
        CudaBackend::acquire()
            .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host."),
    );

    let program = add_one_program();
    let inputs = vec![u32_bytes(&[0, 1, 2, 3, 4, 5, 6, 7])];
    let config = DispatchConfig::default();
    backend.reset_telemetry();

    let _cached = backend
        .record_cuda_graph(&program, &inputs, &config)
        .expect("Fix: cudaGraph recording must succeed for the add-one telemetry contract.");

    let telemetry = backend.telemetry_snapshot();
    assert!(
        telemetry.transient_allocation_bytes_requested >= 64,
        "Fix: cudaGraph recording allocates raw input/output device buffers outside the \
         transient pool; telemetry must include at least the 32-byte input and 32-byte output \
         buffers instead of underreporting CUDA memory pressure. observed={}",
        telemetry.transient_allocation_bytes_requested
    );
    assert_eq!(
        telemetry.sync_points, 1,
        "Fix: cudaGraph recording must account for the parameter-initialization stream synchronization exactly once."
    );
}

#[test]
fn cuda_graph_rejects_input_shape_mismatch() {
    let backend = Arc::new(
        CudaBackend::acquire()
            .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host."),
    );

    let program = add_one_program();
    let inputs = vec![u32_bytes(&[0; 8])];
    let config = DispatchConfig::default();

    let mut cached = backend
        .record_cuda_graph(&program, &inputs, &config)
        .expect("record must succeed");

    // Try replay with WRONG-LENGTH input.
    let bad_inputs = [u32_bytes(&[0; 4])]; // half the recorded size
    let bad_refs: Vec<&[u8]> = bad_inputs.iter().map(Vec::as_slice).collect();
    match backend.dispatch_via_cuda_graph(&mut cached, &bad_refs) {
        Err(BackendError::InvalidProgram { fix }) => {
            assert!(
                fix.contains("re-record") || fix.contains("expects"),
                "rejection error must mention the size mismatch + tell the user to re-record \
                 the graph; got: {fix}"
            );
        }
        Ok(_) => panic!(
            "cuda_graph dispatch must NOT silently accept inputs of the wrong byte length; \
             expected BackendError::InvalidProgram with a structured fix string"
        ),
        Err(other) => panic!(
            "cuda_graph dispatch with mismatched input size must return InvalidProgram, \
             not {other:?}"
        ),
    }
}

#[test]
fn cuda_graph_replay_uses_cached_telemetry_totals_without_per_replay_scans() {
    let replay_source = include_str!("../../src/backend/cuda_graph_replay.rs");
    let graph_source = include_str!("../../src/backend/cuda_graph.rs");

    assert!(
        graph_source.contains("replay_input_bytes")
            && graph_source.contains("replay_output_bytes")
            && graph_source.contains("replay_host_upload_operations")
            && graph_source.contains("replay_device_readback_operations"),
        "Fix: cached CUDA graphs must store fixed-shape replay telemetry totals at record time."
    );
    assert!(
        replay_source.contains("CudaGraphReplayStats::from_cached(cached)"),
        "Fix: CUDA graph replay must reuse cached telemetry totals instead of rebuilding stats."
    );
    assert!(
        !replay_source.contains(".iter()\n                .fold(0_u64")
            && !replay_source.contains(".iter().filter("),
        "Fix: CUDA graph replay must not rescan inputs or output_lens for per-replay telemetry accounting."
    );
    assert!(
        !graph_source.contains("sample_inputs.iter().map(Vec::as_slice).collect()")
            && !graph_source.contains(".map(DevicePtrGuard::into_raw)")
            && !graph_source.contains("device_ptr.saturating_add")
            && !graph_source.contains(concat!(".", "saturating_add")),
        "Fix: CUDA graph recording must avoid iterator collect staging and saturating arithmetic while preparing sample inputs, telemetry totals, and raw device pointers."
    );
    assert!(
        graph_source.contains("cuda_output_readback_for_binding(")
            && !graph_source.contains("program.buffers()[binding.buffer_index]"),
        "Fix: CUDA graph capture readback planning must use the shared checked program-buffer lookup instead of directly indexing program buffers."
    );
    assert!(
        graph_source.contains("fn cuda_graph_sample_input")
            && graph_source.contains(".get(input_index)")
            && graph_source.contains(".copied()")
            && graph_source.contains("expected sample input index {input_index}")
            && !graph_source.contains("sample_inputs[input_index]"),
        "Fix: CUDA graph capture must turn stale binding sample-input indexes into BackendError instead of directly indexing borrowed sample input slices."
    );
}
