//! Adversarial invariants for Program-as-value semantics.

use proptest::prelude::*;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

fn base_buffers() -> Vec<BufferDecl> {
    vec![
        BufferDecl::output("out", 0, DataType::U32)
            .with_count(8)
            .with_output_byte_range(0..16),
        BufferDecl::read("input", 1, DataType::U32).with_count(8),
        BufferDecl::read_write("rw", 2, DataType::U32)
            .with_count(8)
            .with_bytes_extraction(true),
    ]
}

fn reordered_buffers() -> Vec<BufferDecl> {
    vec![
        BufferDecl::read_write("rw", 2, DataType::U32)
            .with_count(8)
            .with_bytes_extraction(true),
        BufferDecl::output("out", 0, DataType::U32)
            .with_count(8)
            .with_output_byte_range(0..16),
        BufferDecl::read("input", 1, DataType::U32).with_count(8),
    ]
}

fn body_for(seed: u32) -> Vec<Node> {
    vec![
        Node::let_bind("idx", Expr::u32(seed % 4)),
        Node::store(
            "out",
            Expr::var("idx"),
            Expr::add(
                Expr::load("input", Expr::var("idx")),
                Expr::load("rw", Expr::var("idx")),
            ),
        ),
        Node::Return,
    ]
}

fn program_for(seed: u32) -> Program {
    Program::wrapped(base_buffers(), [8, 1, 1], body_for(seed))
}

fn reordered_program_for(seed: u32) -> Program {
    Program::wrapped(reordered_buffers(), [8, 1, 1], body_for(seed))
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    #[test]
    fn fingerprint_is_stable_across_clone_and_wire_roundtrip(seed in any::<u32>()) {
        let program = program_for(seed);
        let clone = program.clone();
        let round_tripped = Program::from_wire(
            &program
                .to_wire()
                .expect("Fix: canonical-law fixture must encode"),
        )
        .expect("Fix: canonical-law fixture must decode");

        prop_assert_eq!(program.fingerprint(), clone.fingerprint());
        prop_assert_eq!(program.fingerprint(), round_tripped.fingerprint());
    }

    #[test]
    fn equality_is_symmetric(seed in any::<u32>()) {
        let left = program_for(seed);
        let right = Program::from_wire(
            &left
                .to_wire()
                .expect("Fix: equality fixture must encode"),
        )
        .expect("Fix: equality fixture must decode");

        prop_assert_eq!(left == right, right == left);
    }

    #[test]
    fn wire_serialization_is_idempotent(seed in any::<u32>()) {
        let program = program_for(seed);
        let encoded = program
            .to_wire()
            .expect("Fix: idempotence fixture must encode");
        let reencoded = Program::from_wire(&encoded)
            .expect("Fix: idempotence fixture must decode")
            .to_wire()
            .expect("Fix: decoded idempotence fixture must re-encode");

        prop_assert_eq!(encoded, reencoded);
    }

    #[test]
    fn structural_equality_ignores_buffer_declaration_order(seed in any::<u32>()) {
        let left = program_for(seed);
        let right = reordered_program_for(seed);

        prop_assert_eq!(
            left,
            right,
            "Fix: Program equality must treat buffer declarations as a semantic set, not an ordered list"
        );
    }
}
