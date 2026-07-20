//! Differential contracts for every registered dual CPU oracle.
//!
//! The reference backend is the correctness oracle for GPU parity. Every
//! operation with two independently-written references must stay registered
//! and must agree on hostile byte patterns, not only happy-path fixtures.

use std::collections::BTreeSet;

use vyre_reference::{dual_op_ids, resolve_dual};

const ADVERSARIAL_INPUTS: &[&[u8]] = &[
    &[],
    &[0x00],
    &[0xFF],
    &[0x00, 0xFF, 0x55, 0xAA],
    &[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x7F, 0x80],
    &[
        0x00, 0x01, 0x02, 0x03, 0x7E, 0x7F, 0x80, 0x81, 0xFC, 0xFD, 0xFE, 0xFF,
    ],
];

#[test]
fn every_dual_reference_id_is_unique_registered_and_differentially_equal() {
    let ids = dual_op_ids();
    assert!(
        !ids.is_empty(),
        "Fix: the reference oracle must expose at least one dual-reference op for differential checks."
    );

    let mut unique = BTreeSet::new();
    for &op_id in ids {
        assert!(
            unique.insert(op_id),
            "Fix: dual_op_ids() must not contain duplicate op id `{op_id}`."
        );
        let (reference_a, reference_b) = resolve_dual(op_id)
            .unwrap_or_else(|| panic!("Fix: dual op `{op_id}` must resolve to both references."));

        for input in ADVERSARIAL_INPUTS {
            let left = reference_a(input);
            let right = reference_b(input);
            assert_eq!(
                left, right,
                "Fix: dual references for `{op_id}` disagree on input bytes {input:02x?}."
            );
            assert_eq!(
                left.len(),
                4,
                "Fix: scalar dual reference `{op_id}` must emit exactly one little-endian u32."
            );
        }
    }
}

#[test]
fn unknown_dual_reference_id_stays_absent() {
    assert!(
        resolve_dual("vendor.unknown.dual").is_none(),
        "Fix: unknown dual-reference ids must not resolve to a fallback oracle."
    );
}
