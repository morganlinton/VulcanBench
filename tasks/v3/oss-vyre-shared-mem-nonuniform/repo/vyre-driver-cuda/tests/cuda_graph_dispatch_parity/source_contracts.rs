#[test]
fn cuda_graph_uses_nonblocking_dedicated_stream() {
    let source = include_str!("../../src/backend/cuda_graph.rs");
    assert!(
        source.contains("CU_STREAM_NON_BLOCKING"),
        "Fix: CUDA graph capture/replay must use a nonblocking dedicated stream, not CUDA's legacy-default-stream-ordered blocking stream."
    );
    assert!(
        !source.contains("cuStreamCreate(&mut stream_ptr, 0)"),
        "Fix: CUDA graph dedicated stream creation must not pass flag 0; that can inherit unwanted default-stream ordering."
    );
}

#[test]
fn cuda_graph_capture_does_not_allocate_fake_empty_param_buffer() {
    let source = include_str!("../../src/backend/cuda_graph.rs");
    assert!(
        source.contains("if param_bytes != 0 {\n            // SAFETY: param_bytes is u32-aligned and non-zero in this branch."),
        "Fix: CUDA graph capture must use a null parameter pointer for empty launch params instead of allocating a rounded 1-byte buffer."
    );
    assert!(
        !source.contains("cuMemAlloc_v2(&mut params_device_ptr, param_bytes.max(1))")
            && !source.contains("record_transient_allocation_bytes(param_bytes.max(1) as u64)"),
        "Fix: CUDA graph parameter capture must not hide empty launch params behind max(1) allocation or telemetry."
    );
}

#[test]
fn cuda_graph_param_initialization_sync_is_telemetry_visible() {
    let source = include_str!("../../src/backend/cuda_graph.rs");
    assert!(
        source.contains("cuStreamSynchronize(stream.ptr().as_ptr())")
            && source.contains("self.telemetry.record_sync_point();"),
        "Fix: CUDA graph parameter initialization must record its stream synchronization in telemetry."
    );
}

