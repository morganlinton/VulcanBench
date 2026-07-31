//! Bounded resource-exhaustion adversarial tests.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::validate::validate;

#[test]
fn many_buffer_program_roundtrips_without_wire_blowup() {
    let buffers = (0..256)
        .map(|index| {
            let name = format!("buf_{index}");
            BufferDecl::output(&name, index, DataType::U32).with_count(4)
        })
        .collect::<Vec<_>>();
    let program = Program::wrapped(
        buffers,
        [64, 1, 1],
        vec![Node::store("buf_0", Expr::u32(0), Expr::u32(1))],
    );

    let encoded = program.to_wire().expect("many-buffer program must encode");
    assert!(
        encoded.len() < 2_000_000,
        "many-buffer program wire size must remain bounded, got {} bytes",
        encoded.len()
    );
    let decoded = Program::from_wire(&encoded).expect("many-buffer program must decode");
    assert_eq!(decoded.fingerprint(), program.fingerprint());
}

#[test]
fn many_node_program_validates_and_roundtrips() {
    let nodes = (0..2048)
        .map(|index| Node::let_bind(format!("x_{index}"), Expr::u32(index as u32)))
        .chain(std::iter::once(Node::store(
            "out",
            Expr::u32(0),
            Expr::u32(7),
        )))
        .collect::<Vec<_>>();
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [128, 1, 1],
        nodes,
    );

    let errors = validate(&program);
    assert!(
        errors.is_empty(),
        "many-node program must remain valid, got {errors:?}"
    );
    let encoded = program.to_wire().expect("many-node program must encode");
    let decoded = Program::from_wire(&encoded).expect("many-node program must decode");
    assert_eq!(decoded.fingerprint(), program.fingerprint());
}

#[test]
fn truncated_wire_input_is_rejected_without_panic() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let encoded = program.to_wire().expect("program must encode");

    for len in 0..encoded.len().min(16) {
        assert_eq!(
            Program::from_wire(&encoded[..len]).is_err(),
            true,
            "truncated wire prefix of length {len} must be rejected"
        );
    }
}
