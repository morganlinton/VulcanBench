//! Adversarial memory-ordering tests.

use std::collections::BTreeSet;

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre::MemoryOrdering;

#[test]
fn memory_ordering_wire_tags_are_total_and_unique_for_known_values() {
    let orderings = [
        MemoryOrdering::Relaxed,
        MemoryOrdering::Acquire,
        MemoryOrdering::Release,
        MemoryOrdering::AcqRel,
        MemoryOrdering::SeqCst,
        MemoryOrdering::GridSync,
    ];
    let tags = orderings
        .iter()
        .map(|ordering| ordering.wire_tag())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        tags.len(),
        orderings.len(),
        "every memory ordering must have a unique wire tag"
    );
    for ordering in orderings {
        assert_eq!(
            MemoryOrdering::from_wire_tag(ordering.wire_tag()),
            Ok(ordering),
            "known memory ordering tags must round-trip"
        );
    }
}

#[test]
fn unknown_memory_ordering_wire_tags_fail_closed() {
    for tag in [7, 31, 127, 255] {
        assert_eq!(
            MemoryOrdering::from_wire_tag(tag).is_err(),
            true,
            "unknown memory ordering tag {tag} must be rejected"
        );
    }
}

#[test]
fn barrier_ordering_survives_program_wire_roundtrip() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [64, 1, 1],
        vec![
            Node::barrier_with_ordering(MemoryOrdering::SeqCst),
            Node::store("out", Expr::u32(0), Expr::u32(1)),
        ],
    );

    let encoded = program.to_wire().expect("barrier program must encode");
    let decoded = Program::from_wire(&encoded).expect("barrier program must decode");

    assert_eq!(
        decoded.fingerprint(),
        program.fingerprint(),
        "barrier ordering must remain structural across wire round-trip"
    );
}
