//! Regression test for FIX-REVIEW Finding #1:
//! Wire format must preserve `non_composable_with_self` on round-trip.

use vyre_foundation::ir::{BufferDecl, DataType, Node, Program};

#[test]
fn non_composable_with_self_true_roundtrips() {
    let original = Program::wrapped(
        vec![
            BufferDecl::read("in", 0, DataType::U32),
            BufferDecl::read_write("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    )
    .with_non_composable_with_self(true);

    let encoded = original
        .to_wire()
        .expect("Fix: non_composable=true program must encode");
    let decoded =
        Program::from_wire(&encoded).expect("Fix: non_composable=true program must decode");

    assert!(
        decoded.is_non_composable_with_self(),
        "Fix: decoded program must preserve non_composable_with_self=true"
    );
    assert!(
        decoded.structural_eq(&original),
        "Fix: decoded program must be structurally equal to original"
    );
}

#[test]
fn non_composable_with_self_false_roundtrips() {
    let original = Program::wrapped(
        vec![
            BufferDecl::read("in", 0, DataType::U32),
            BufferDecl::read_write("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    )
    .with_non_composable_with_self(false);

    let encoded = original
        .to_wire()
        .expect("Fix: non_composable=false program must encode");
    let decoded =
        Program::from_wire(&encoded).expect("Fix: non_composable=false program must decode");

    assert!(
        !decoded.is_non_composable_with_self(),
        "Fix: decoded program must preserve non_composable_with_self=false"
    );
    assert!(
        decoded.structural_eq(&original),
        "Fix: decoded program must be structurally equal to original"
    );
}
