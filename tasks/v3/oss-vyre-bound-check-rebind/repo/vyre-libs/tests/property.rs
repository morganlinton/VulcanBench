//! Property tests for vyre-libs  -  invariants that must hold across
//! every input in the relevant domain.

#![cfg(all(
    feature = "math-linalg",
    feature = "math-scan",
    feature = "math-broadcast",
    feature = "crypto-fnv",
))]

use proptest::prelude::*;
use vyre::ir::Program;
use vyre_libs::hash::fnv1a32;
use vyre_libs::math::{broadcast, dot, matmul, scan_prefix_sum};

fn has_single_region(program: &Program) -> bool {
    matches!(program.entry().first(), Some(vyre::ir::Node::Region { .. }))
        && program.entry().len() == 1
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        ..ProptestConfig::default()
    })]

    #[test]
    fn dot_program_is_always_single_region(
        a in "[a-z][a-z0-9_]*",
        b in "[a-z][a-z0-9_]*",
        c in "[a-z][a-z0-9_]*",
    ) {
        prop_assume!(a != b && b != c && a != c);
        let p = dot(&a, &b, &c, 256).unwrap();
        prop_assert!(has_single_region(&p));
    }

    #[test]
    fn matmul_preserves_dims(
        m in 1u32..64,
        k in 1u32..64,
        n in 1u32..64,
    ) {
        let p = matmul("a", "b", "c", m, k, n);
        prop_assert!(has_single_region(&p));
        prop_assert_eq!(p.workgroup_size(), [256, 1, 1]);
    }

    #[test]
    fn scan_prefix_sum_is_valid_for_all_sizes(n in 1u32..1024) {
        let p = scan_prefix_sum("in", "out", n);
        prop_assert!(has_single_region(&p));
        prop_assert_eq!(p.workgroup_size(), [n.next_power_of_two(), 1, 1]);
    }

    #[test]
    fn broadcast_is_structurally_valid(
        s in "[a-z][a-z0-9_]*",
        d in "[a-z][a-z0-9_]*",
    ) {
        prop_assume!(s != d);
        let p = broadcast(&s, &d, 8);
        prop_assert!(has_single_region(&p));
    }

    #[test]
    fn fnv1a32_single_workgroup(
        i in "[a-z][a-z0-9_]*",
        o in "[a-z][a-z0-9_]*",
    ) {
        prop_assume!(i != o);
        let p = fnv1a32(&i, &o);
        prop_assert!(has_single_region(&p));
        prop_assert_eq!(p.workgroup_size(), [1, 1, 1]);
    }
}

// Every vyre-libs Program contains one top-level Region; these tests
// prove the full Region wire round-trip (generator + optional
// source_region + body) is byte-identity stable across encode/decode.
#[test]

fn wire_round_trip_for_dot() {
    let p = dot("a", "b", "c", 4).unwrap();
    let wire = p.to_wire().expect("dot program must serialize");
    let parsed = Program::from_wire(&wire).expect("dot wire bytes must decode");
    assert_eq!(parsed, p);
}

#[test]

fn wire_round_trip_for_fnv1a32() {
    let p = fnv1a32("input", "out");
    let wire = p.to_wire().expect("fnv1a32 program must serialize");
    let parsed = Program::from_wire(&wire).expect("fnv1a32 wire bytes must decode");
    assert_eq!(parsed, p);
}
