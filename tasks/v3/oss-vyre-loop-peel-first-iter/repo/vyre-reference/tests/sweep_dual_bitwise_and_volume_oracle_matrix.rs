//! Volume oracle matrix - independent reference vs production cpu_ref.
//! Legendary testing.volume - do NOT weaken to shape-only asserts.
#![forbid(unsafe_code)]

use vyre_reference::{dual_op_ids, resolve_dual};

const CASES: u32 = 16384;
const OP_ID: &str = "primitive.bitwise.and";

fn hostile_pair(seed: u32) -> (u32, u32) {
    let left = seed
        .wrapping_mul(0x85eb_ca6b)
        .rotate_left((seed ^ 0x13) & 31);
    let right = seed
        .wrapping_mul(0xc2b2_ae35)
        .rotate_right((seed ^ 0x29) & 31);
    (left, right)
}

fn binary_input(left: u32, right: u32) -> Vec<u8> {
    let mut input = Vec::with_capacity(8);
    input.extend_from_slice(&left.to_le_bytes());
    input.extend_from_slice(&right.to_le_bytes());
    input
}

#[test]
fn sweep_dual_bitwise_and_volume_oracle_matrix() {
    assert!(
        dual_op_ids().contains(&OP_ID),
        "Fix: {OP_ID} must stay registered"
    );
    let (reference_a, reference_b) = resolve_dual(OP_ID).expect("Fix: dual reference must resolve");
    for seed in 0..CASES {
        let (left, right) = hostile_pair(seed);
        let input = binary_input(left, right);
        let expected = (left & right).to_le_bytes().to_vec();
        let output_a = reference_a(&input);
        let output_b = reference_b(&input);
        assert_eq!(
            output_a, output_b,
            "Fix: {OP_ID} dual refs diverged seed={seed}"
        );
        assert_eq!(
            output_a, expected,
            "Fix: {OP_ID} volume oracle mismatch seed={seed} left={left:#010x} right={right:#010x}"
        );
    }
}
