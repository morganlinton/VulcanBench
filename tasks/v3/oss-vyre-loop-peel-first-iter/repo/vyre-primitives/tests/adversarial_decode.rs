//! Failure-oriented adversarial tests for decode primitives.
//!
//! Focus: hostile boundaries, overflow, invalid offsets, property invariants.
#![cfg(feature = "decode")]

use vyre_primitives::decode::base64::*;

#[test]
fn decoded_capacity_hostile_lengths() {
    let max_blocks = u32::MAX / 4;
    let max_expected = max_blocks.saturating_mul(3);
    let cases = [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 0),
        (4, 3),
        (5, 3),
        (7, 3),
        (8, 6),
        (u32::MAX, max_expected),
    ];
    for (input_len, expected) in cases {
        let got = decoded_capacity(input_len);
        assert_eq!(got, expected, "decoded_capacity({input_len}) mismatch");
    }
}

#[test]
fn decoded_capacity_no_panic_on_max() {
    let _ = decoded_capacity(u32::MAX);
}

#[test]
fn base64_decode_program_has_expected_buffers() {
    let p = base64_decode("input", "table", "output", "decoded_len", 4);
    let names: Vec<&str> = p.buffers().iter().map(|b| b.name()).collect();
    assert_eq!(names, vec!["input", "table", "output", "decoded_len"]);
}

#[test]
fn base64_decode_child_returns_region() {
    let node = base64_decode_child("parent", "input", "table", "output", "decoded_len", 4);
    match node {
        vyre_foundation::ir::Node::Region { generator, .. } => {
            assert_eq!(generator.as_str(), BASE64_DECODE_OP_ID);
        }
        other => panic!("expected Region node, got {other:?}"),
    }
}
