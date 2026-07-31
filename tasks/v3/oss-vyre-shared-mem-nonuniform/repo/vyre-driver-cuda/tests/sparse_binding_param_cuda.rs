//! Live CUDA parity for sparse binding-slot launch parameters.

mod common;

use common::{bytes_u32, cuda_reference_outputs, live_backend, u32_bytes};
use vyre_driver::DispatchConfig;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

#[test]
fn cuda_dynamic_buffer_lengths_are_indexed_by_binding_slot_not_buffer_index() {
    let backend = live_backend();
    let input = [3, 1, 4, 1, 5, 9, 2, 6];
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 9, DataType::U32),
            BufferDecl::output("out", 2, DataType::U32).with_count(input.len() as u32),
        ],
        [64, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(10)),
        )],
    );

    let outputs = cuda_reference_outputs(
        &backend,
        &program,
        &[u32_bytes(&input)],
        "sparse_binding_slot_param_words",
    );
    let expected = input.iter().map(|value| value + 10).collect::<Vec<_>>();

    for (path, buffers) in [
        ("direct CUDA", &outputs.direct_cuda),
        ("compiled CUDA", &outputs.compiled_cuda),
        ("reference", &outputs.reference),
    ] {
        assert_eq!(
            bytes_u32(&buffers[0]),
            expected,
            "Fix: {path} must read dynamic input length from the binding-slot parameter word, not from the Program buffer index."
        );
    }
}

#[test]
fn cuda_sparse_output_binding_slot_keeps_store_bounds_and_readback_correct() {
    let backend = live_backend();
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 11, DataType::U32).with_count(6)],
        [64, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::mul(Expr::gid_x(), Expr::u32(7)),
        )],
    );

    let outputs = backend
        .dispatch(&program, &[], &DispatchConfig::default())
        .expect("Fix: CUDA must dispatch sparse output binding-slot programs without param-word ABI truncation.");
    assert_eq!(
        bytes_u32(&outputs[0]),
        vec![0, 7, 14, 21, 28, 35],
        "Fix: CUDA output stores must use the sparse output binding slot's length metadata."
    );
}
