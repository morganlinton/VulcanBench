use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Program};

use super::{dispatch_param_words_into, infer_dispatch_grid};
use crate::backend::DispatchConfig;
use crate::binding::{Binding, BindingRole};
use vyre_foundation::ir::{Expr, Node};

fn singleton_atomic_byte_flag_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("bytes_in", 0, BufferAccess::ReadOnly, DataType::U8).with_count(0),
            BufferDecl::storage("flag", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [256, 1, 1],
        vec![Node::let_bind(
            "flag_old",
            Expr::atomic_or(
                "flag",
                Expr::u32(0),
                Expr::load("bytes_in", Expr::InvocationId { axis: 0 }),
            ),
        )],
    )
}

#[test]
fn infer_grid_uses_readonly_input_for_accumulator_kernels() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("accum", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage("values", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1_000_000),
        ],
        [256, 1, 1],
        vec![Node::let_bind(
            "_",
            Expr::atomic_add(
                "accum",
                Expr::u32(0),
                Expr::load("values", Expr::InvocationId { axis: 0 }),
            ),
        )],
    );
    let inputs = vec![vec![0u8; 4], vec![0u8; 1_000_000 * 4]];

    let grid = infer_dispatch_grid(&program, &inputs, &DispatchConfig::default())
        .expect("Fix: accumulator kernel grid should infer from full binding plan");

    assert_eq!(grid, [3907, 1, 1]);
}

#[test]
fn infer_grid_uses_dynamic_byte_input_for_singleton_atomic_flags() {
    let program = singleton_atomic_byte_flag_program();
    let inputs = vec![vec![0u8; 4097], vec![0u8; 4]];

    let grid = infer_dispatch_grid(&program, &inputs, &DispatchConfig::default())
        .expect("Fix: singleton atomic flags must still dispatch across the dynamic byte input.");

    assert_eq!(grid, [17, 1, 1]);
}

#[test]
fn generated_dynamic_byte_singleton_atomic_flag_grid_matrix() {
    let program = singleton_atomic_byte_flag_program();
    let mut inputs = vec![Vec::new(), vec![0u8; 4]];
    let mut checked = 0u32;

    for byte_len in 1..=10_000u32 {
        inputs[0].resize(byte_len as usize, 0);
        let grid = infer_dispatch_grid(&program, &inputs, &DispatchConfig::default())
            .expect("Fix: generated singleton-flag grid inference should accept byte inputs.");

        assert_eq!(
            grid,
            [byte_len.div_ceil(256), 1, 1],
            "Fix: dynamic byte length {byte_len} must drive singleton atomic flag dispatch."
        );
        checked += 1;
    }

    assert_eq!(
        checked, 10_000,
        "Fix: generated singleton-flag grid matrix must cover ten thousand byte lengths."
    );
}

#[test]
fn infer_grid_prefers_explicit_output_over_large_inputs() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1_000_000),
            BufferDecl::output("out", 1, DataType::U32).with_count(512),
        ],
        [256, 1, 1],
        vec![Node::store(
            "out",
            Expr::InvocationId { axis: 0 },
            Expr::load("input", Expr::InvocationId { axis: 0 }),
        )],
    );
    let inputs = vec![vec![0u8; 1_000_000 * 4]];

    let grid = infer_dispatch_grid(&program, &inputs, &DispatchConfig::default())
        .expect("Fix: output-driven kernel grid should infer from the output binding");

    assert_eq!(grid, [2, 1, 1]);
}

#[test]
fn infer_grid_uses_full_span_for_shared_memory_reductions() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("values", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1024),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
            BufferDecl::workgroup("scratch", 2, DataType::U32).with_count(256),
        ],
        [256, 1, 1],
        vec![],
    );
    let inputs = vec![vec![0u8; 1024 * 4]];

    let grid = infer_dispatch_grid(&program, &inputs, &DispatchConfig::default())
        .expect("Fix: shared-memory reduction grid should cover all input lanes");

    assert_eq!(grid, [4, 1, 1]);
}

#[test]
fn infer_grid_counts_readwrite_live_out_buffers() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(5),
            BufferDecl::storage("histogram", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(256),
            BufferDecl::output("encoding", 2, DataType::U32).with_count(1),
        ],
        [256, 1, 1],
        vec![],
    );
    let inputs = vec![vec![0u8; 5 * 4], vec![0u8; 256 * 4]];

    let grid = infer_dispatch_grid(&program, &inputs, &DispatchConfig::default())
        .expect("Fix: read-write live-out buffers should size the dispatch span");

    assert_eq!(grid, [1, 1, 1]);
}

#[test]
fn dispatch_param_words_into_reuses_output_buffer() {
    let bindings = vec![
        Binding {
            name: std::sync::Arc::from("a"),
            binding: 0,
            buffer_index: 0,
            role: BindingRole::Input,
            element_size: 4,
            preferred_alignment: 4,
            element_count: 7,
            static_byte_len: Some(28),
            input_index: Some(0),
            output_index: None,
        },
        Binding {
            name: std::sync::Arc::from("dynamic"),
            binding: 4,
            buffer_index: 4,
            role: BindingRole::Output,
            element_size: 4,
            preferred_alignment: 4,
            element_count: 0,
            static_byte_len: None,
            input_index: None,
            output_index: Some(0),
        },
    ];
    let mut words = Vec::with_capacity(8);
    let ptr = words.as_ptr();
    dispatch_param_words_into(&bindings, 11, &mut words)
        .expect("Fix: dispatch_param_words_into must succeed for valid bindings");
    assert_eq!(words, vec![11, 7, 0, 0, 0, 11]);
    assert_eq!(words.as_ptr(), ptr);
}
