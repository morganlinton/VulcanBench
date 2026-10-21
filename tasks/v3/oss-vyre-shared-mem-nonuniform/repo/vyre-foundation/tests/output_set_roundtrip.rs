//! OutputSet wire-format round-trip tests.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

#[test]
fn output_set_roundtrip_via_program_wire() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(4),
            BufferDecl::read_write("scratch_out", 2, DataType::U32).with_count(4),
            BufferDecl::storage("write_only", 3, BufferAccess::WriteOnly, DataType::U32)
                .with_count(4),
        ],
        [64, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("input", Expr::u32(0)),
        )],
    );

    let wire = program.to_wire().expect("encode must succeed");
    let decoded = Program::from_wire(&wire).expect("decode must succeed");
    assert_eq!(decoded.output_buffer_indices(), &[1, 2, 3]);
}

#[test]
fn output_set_empty_when_no_writable_buffers() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(4),
            BufferDecl::read("b", 1, DataType::U32).with_count(4),
        ],
        [64, 1, 1],
        vec![],
    );

    let wire = program.to_wire().expect("encode must succeed");
    let decoded = Program::from_wire(&wire).expect("decode must succeed");
    assert!(decoded.output_buffer_indices().is_empty());
}

#[test]
fn output_set_preserves_order_across_declaration_gaps() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read_write("rw0", 0, DataType::U32).with_count(1),
            BufferDecl::read("ro1", 1, DataType::U32).with_count(1),
            BufferDecl::read_write("rw2", 2, DataType::U32).with_count(1),
            BufferDecl::read("ro3", 3, DataType::U32).with_count(1),
            BufferDecl::storage("wo4", 4, BufferAccess::WriteOnly, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![],
    );

    let wire = program.to_wire().expect("encode must succeed");
    let decoded = Program::from_wire(&wire).expect("decode must succeed");
    assert_eq!(decoded.output_buffer_indices(), &[0, 2, 4]);
    assert_eq!(decoded.buffers()[4].access(), BufferAccess::WriteOnly);
}
