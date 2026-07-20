use super::*;

#[test]
fn cuda_graph_dispatch_matches_direct_dispatch_byte_for_byte() {
    let backend = Arc::new(
        CudaBackend::acquire()
            .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host."),
    );

    let program = add_one_program();
    let initial_inputs = vec![u32_bytes(&[0, 1, 2, 3, 4, 5, 6, 7])];
    let config = DispatchConfig::default();

    // Record the graph once with sample inputs.
    let mut cached = backend
        .record_cuda_graph(&program, &initial_inputs, &config)
        .expect("Fix: cudaGraph recording must succeed for the trivial add-one program");

    // Run direct dispatch as the parity oracle.
    let direct_outputs = backend
        .dispatch(&program, &initial_inputs, &config)
        .expect("direct dispatch must succeed for parity comparison");

    // Run via cached graph with the SAME inputs; outputs must match byte-for-byte.
    let input_refs: Vec<&[u8]> = initial_inputs.iter().map(Vec::as_slice).collect();
    let graph_outputs = backend
        .dispatch_via_cuda_graph(&mut cached, &input_refs)
        .expect("cuda_graph replay must succeed");

    assert_eq!(
        direct_outputs.len(),
        graph_outputs.len(),
        "output buffer count must match between direct dispatch and graph replay"
    );
    assert_eq!(
        bytes_u32(&direct_outputs[0]),
        bytes_u32(&graph_outputs[0]),
        "direct dispatch and graph replay must produce byte-identical outputs"
    );
    let mut reusable_outputs = Vec::with_capacity(graph_outputs.len());
    backend
        .dispatch_via_cuda_graph_into(&mut cached, &input_refs, &mut reusable_outputs)
        .expect("cuda_graph replay into reusable output buffer must succeed");
    let reusable_capacity = reusable_outputs.capacity();
    assert_eq!(
        bytes_u32(&direct_outputs[0]),
        bytes_u32(&reusable_outputs[0]),
        "reusable graph replay output must match direct dispatch byte-for-byte"
    );
    let reusable_inner_capacity = reusable_outputs[0].capacity();
    backend
        .dispatch_via_cuda_graph_into(&mut cached, &input_refs, &mut reusable_outputs)
        .expect("second reusable graph replay must succeed");
    assert_eq!(
        reusable_outputs.capacity(),
        reusable_capacity,
        "dispatch_via_cuda_graph_into must reuse the caller's outer output Vec allocation"
    );
    assert_eq!(
        reusable_outputs[0].capacity(),
        reusable_inner_capacity,
        "dispatch_via_cuda_graph_into must reuse each existing output byte buffer allocation"
    );

    // Try a SECOND replay with different input bytes  -  same shape, new data.
    let new_inputs = vec![u32_bytes(&[100, 200, 300, 400, 500, 600, 700, 800])];
    let direct_outputs_2 = backend
        .dispatch(&program, &new_inputs, &config)
        .expect("second direct dispatch must succeed");
    let new_input_refs: Vec<&[u8]> = new_inputs.iter().map(Vec::as_slice).collect();
    let graph_outputs_2 = backend
        .dispatch_via_cuda_graph(&mut cached, &new_input_refs)
        .expect("second graph replay must succeed");
    assert_eq!(
        bytes_u32(&direct_outputs_2[0]),
        bytes_u32(&graph_outputs_2[0]),
        "graph replay with NEW inputs must produce the SAME outputs as direct dispatch on \
         those inputs  -  without this, the cached host buffer write isn't being picked up by \
         the captured memcpy on replay"
    );
}

#[test]
fn cuda_graph_bool_storage_abi_matches_direct_dispatch() {
    let backend = Arc::new(
        CudaBackend::acquire()
            .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host."),
    );
    let program = bool_not_program();
    let inputs = vec![bool_bytes(&[
        false, true, true, false, true, false, false, true,
    ])];
    let config = DispatchConfig::default();
    let direct_outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("Fix: direct CUDA Bool dispatch must succeed before cudaGraph parity.");
    let mut cached = backend
        .record_cuda_graph(&program, &inputs, &config)
        .expect("Fix: cudaGraph recording must support Bool word-ABI inputs and outputs.");
    let input_refs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
    let graph_outputs = backend
        .dispatch_via_cuda_graph(&mut cached, &input_refs)
        .expect("Fix: cudaGraph replay must support Bool word-ABI inputs and outputs.");

    assert_eq!(
        direct_outputs, graph_outputs,
        "Fix: cudaGraph Bool replay must match direct CUDA dispatch byte-for-byte."
    );
    assert_eq!(
        bytes_u32(&graph_outputs[0]),
        vec![1, 0, 0, 1, 0, 1, 1, 0],
        "Fix: cudaGraph Bool output must use the stable one-u32-word-per-lane ABI."
    );
}

#[test]
fn cuda_graph_honors_output_byte_ranges_like_direct_dispatch() {
    let backend = Arc::new(
        CudaBackend::acquire()
            .expect("Fix: CUDA backend acquisition must succeed on the GPU-required test host."),
    );
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("state", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(4)
                .with_output_byte_range(4..12),
        ],
        [1, 1, 1],
        vec![Node::store("state", Expr::u32(3), Expr::u32(99))],
    );
    let inputs = vec![u32_bytes(&[11, 22, 33, 44])];
    let config = DispatchConfig::default();
    let direct_outputs = backend
        .dispatch(&program, &inputs, &config)
        .expect("Fix: direct CUDA dispatch must accept output byte ranges.");
    let mut cached = backend
        .record_cuda_graph(&program, &inputs, &config)
        .expect("Fix: cudaGraph recording must preserve output byte ranges.");
    let input_refs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
    let graph_outputs = backend
        .dispatch_via_cuda_graph(&mut cached, &input_refs)
        .expect("Fix: cudaGraph replay must preserve output byte ranges.");

    assert_eq!(
        direct_outputs, graph_outputs,
        "Fix: cudaGraph output readback must use the same byte range as direct CUDA dispatch."
    );
    assert_eq!(
        bytes_u32(&graph_outputs[0]),
        vec![22, 33],
        "Fix: cudaGraph output_byte_range=4..12 must return only the requested middle words."
    );
}

