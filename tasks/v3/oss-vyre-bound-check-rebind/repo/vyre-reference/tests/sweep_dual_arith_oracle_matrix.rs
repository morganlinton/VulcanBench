//! Sweep oracle matrix for arithmetic dual-reference contracts.
//!
//! Expands the handwritten dual-arith contracts to 512+ hostile inputs per
//! supported op family, comparing both registered references against an
//! independent Rust wrapping-arithmetic oracle.

#![forbid(unsafe_code)]

use vyre_reference::dual_impls::arith::add::{reference_a as add_a, reference_b as add_b};
use vyre_reference::dual_impls::arith::mul::{reference_a as mul_a, reference_b as mul_b};
use vyre_reference::{dual_op_ids, resolve_dual};

const CASES_PER_FAMILY: u32 = 512;

fn binary_input(left: u32, right: u32) -> Vec<u8> {
    let mut input = Vec::with_capacity(8);
    input.extend_from_slice(&left.to_le_bytes());
    input.extend_from_slice(&right.to_le_bytes());
    input
}

fn hostile_pair(seed: u32) -> (u32, u32) {
    let left = seed
        .wrapping_mul(0x85eb_ca6b)
        .rotate_left((seed ^ 0x13) & 31);
    let right = seed
        .wrapping_mul(0xc2b2_ae35)
        .rotate_right((seed ^ 0x29) & 31);
    (left, right)
}

#[test]
fn sweep_dual_arith_add_oracle_matrix_matches_independent_wrapping_add() {
    assert!(
        dual_op_ids().contains(&"primitive.arith.add"),
        "Fix: primitive.arith.add must remain registered in dual_op_ids()."
    );
    let (reference_a, reference_b) = resolve_dual("primitive.arith.add")
        .expect("Fix: primitive.arith.add must resolve to dual references.");

    let mut assertions = 0usize;
    for seed in 0..CASES_PER_FAMILY {
        let (left, right) = hostile_pair(seed);
        let input = binary_input(left, right);
        let expected = left.wrapping_add(right).to_le_bytes().to_vec();

        let output_a = reference_a(&input);
        let output_b = reference_b(&input);
        assert_eq!(
            output_a, output_b,
            "Fix: add dual references diverged for left={left:#010x} right={right:#010x}"
        );
        assert_eq!(
            output_a, expected,
            "Fix: add dual oracle returned wrong wrapping result for left={left:#010x} right={right:#010x}"
        );

        assert_eq!(
            add_a::reference(&input),
            add_b::reference(&input),
            "Fix: add independent references diverged for left={left:#010x} right={right:#010x}"
        );
        assert_eq!(
            output_a,
            add_b::reference(&input),
            "Fix: registered add dual A diverged from independent bit oracle for left={left:#010x} right={right:#010x}"
        );
        assertions += 4;
    }
    assert_eq!(assertions, CASES_PER_FAMILY as usize * 4);
}

#[test]
fn sweep_dual_arith_mul_oracle_matrix_matches_independent_wrapping_mul() {
    assert!(
        dual_op_ids().contains(&"primitive.arith.mul"),
        "Fix: primitive.arith.mul must remain registered in dual_op_ids()."
    );
    let (reference_a, reference_b) = resolve_dual("primitive.arith.mul")
        .expect("Fix: primitive.arith.mul must resolve to dual references.");

    let mut assertions = 0usize;
    for seed in 0..CASES_PER_FAMILY {
        let (left, right) = hostile_pair(seed ^ 0xA5A5_5A5A);
        let input = binary_input(left, right);
        let expected = left.wrapping_mul(right).to_le_bytes().to_vec();

        let output_a = reference_a(&input);
        let output_b = reference_b(&input);
        assert_eq!(
            output_a, output_b,
            "Fix: mul dual references diverged for left={left:#010x} right={right:#010x}"
        );
        assert_eq!(
            output_a, expected,
            "Fix: mul dual oracle returned wrong wrapping result for left={left:#010x} right={right:#010x}"
        );

        assert_eq!(
            mul_a::reference(&input),
            mul_b::reference(&input),
            "Fix: mul independent references diverged for left={left:#010x} right={right:#010x}"
        );
        assert_eq!(
            output_a,
            mul_b::reference(&input),
            "Fix: registered mul dual A diverged from independent shift oracle for left={left:#010x} right={right:#010x}"
        );
        assertions += 4;
    }
    assert_eq!(assertions, CASES_PER_FAMILY as usize * 4);
}

#[test]
fn sweep_dual_arith_malformed_inputs_zero_fill_without_divergence() {
    let malformed: [&[u8]; 3] = [&[], &[0xFF], &[0x01, 0x02, 0x03, 0x04]];
    let mut assertions = 0usize;
    for op_id in ["primitive.arith.add", "primitive.arith.mul"] {
        let (reference_a, reference_b) =
            resolve_dual(op_id).unwrap_or_else(|| panic!("Fix: {op_id} must be registered."));
        for input in malformed {
            let zero = vec![0; 4];
            assert_eq!(
                reference_a(input),
                zero,
                "Fix: {op_id} ref_a short-input contract drifted"
            );
            assert_eq!(
                reference_b(input),
                zero,
                "Fix: {op_id} ref_b short-input contract drifted"
            );
            assert_ne!(
                reference_a(input),
                vec![0xFF; 4],
                "Fix: {op_id} must not emit sentinel garbage on short input"
            );
            assertions += 3;
        }
    }
    assert_eq!(assertions, 2 * malformed.len() * 3);
}
