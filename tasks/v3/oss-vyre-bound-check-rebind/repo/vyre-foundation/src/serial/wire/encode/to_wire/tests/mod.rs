use super::*;
use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

#[test]
fn to_wire_into_appends_byte_for_byte() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read_write("a", 0, DataType::U32),
            BufferDecl::read("b", 1, DataType::U32),
        ],
        [64, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::store("a", Expr::var("idx"), Expr::load("b", Expr::var("idx"))),
        ],
    );

    let mut separate = Vec::new();
    for _ in 0..100 {
        separate.extend_from_slice(&to_wire(&program).unwrap());
    }

    let mut reused = Vec::new();
    for _ in 0..100 {
        to_wire_into(&program, &mut reused).unwrap();
    }

    assert_eq!(
        separate, reused,
        "100 separate to_wire calls must match 100 to_wire_into calls into the same buffer"
    );
}

#[test]
fn encode_section_helpers_reuse_caller_scratch() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read_write("a", 0, DataType::U32).with_count(64),
            BufferDecl::read("b", 1, DataType::U32).with_count(64),
            BufferDecl::read("mask", 2, DataType::Bool).with_count(64),
        ],
        [64, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::store("a", Expr::var("idx"), Expr::load("b", Expr::var("idx"))),
        ],
    );

    let mut out = Vec::with_capacity(2048);
    let mut payload = Vec::with_capacity(2048);
    put_nodes_section_with_payload(&mut out, &program, program.buffers(), &mut payload)
        .expect("Fix: node section must encode");
    let payload_ptr = payload.as_ptr();
    let payload_capacity = payload.capacity();
    out.clear();
    put_nodes_section_with_payload(&mut out, &program, program.buffers(), &mut payload)
        .expect("Fix: node section must encode a second time");
    assert_eq!(payload.as_ptr(), payload_ptr);
    assert_eq!(payload.capacity(), payload_capacity);

    let mut shape = Vec::with_capacity(64);
    let mut hints = Vec::with_capacity(64);
    put_memory_regions_with_scratch(&mut out, program.buffers(), &mut shape, &mut hints)
        .expect("Fix: memory regions must encode");
    let shape_ptr = shape.as_ptr();
    let hints_ptr = hints.as_ptr();
    let shape_capacity = shape.capacity();
    let hints_capacity = hints.capacity();
    out.clear();
    put_memory_regions_with_scratch(&mut out, program.buffers(), &mut shape, &mut hints)
        .expect("Fix: memory regions must encode a second time");
    assert_eq!(shape.as_ptr(), shape_ptr);
    assert_eq!(hints.as_ptr(), hints_ptr);
    assert_eq!(shape.capacity(), shape_capacity);
    assert_eq!(hints.capacity(), hints_capacity);
}

#[test]
fn output_set_is_serialized_and_validated() {
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

    let encoded = to_wire(&program).expect("Fix: output-set program must encode");
    assert_eq!(
        &encoded[encoded.len() - 4..],
        &[3, 1, 2, 3],
        "OutputSet must list writable buffer indices in declaration order"
    );
    let decoded =
        Program::from_wire(&encoded).expect("Fix: encoded output-set program must decode");
    assert_eq!(decoded.output_buffer_indices(), &[1, 2, 3]);

    let mut tampered = encoded;
    let last = tampered.len() - 1;
    tampered[last] = 0;
    let digest = blake3::hash(&tampered[40..]);
    tampered[8..40].copy_from_slice(digest.as_bytes());
    let err = Program::from_wire(&tampered)
        .expect_err("tampered output-set must be rejected")
        .to_string();
    assert!(
        err.contains("output-set"),
        "decode error must name the corrupt OutputSet: {err}"
    );
}
