//! Arithmetic dual-reference contracts.

use vyre_reference::{dual_op_ids, resolve_dual};

fn binary_input(left: u32, right: u32) -> Vec<u8> {
    let mut input = Vec::with_capacity(8);
    input.extend_from_slice(&left.to_le_bytes());
    input.extend_from_slice(&right.to_le_bytes());
    input
}

#[test]
fn arithmetic_dual_references_are_registered_in_the_public_oracle() {
    let ids = dual_op_ids();
    for op_id in ["primitive.arith.add", "primitive.arith.mul"] {
        assert!(
            ids.contains(&op_id),
            "Fix: {op_id} has dual implementations but is missing from dual_op_ids()."
        );
        assert!(
            resolve_dual(op_id).is_some(),
            "Fix: {op_id} must resolve to both arithmetic CPU references."
        );
    }
}

#[test]
fn generated_arithmetic_dual_matrix_matches_wrapping_u32_contracts() {
    let mut assertions = 0usize;
    for seed in 0..8192u32 {
        let left = seed
            .wrapping_mul(0x85eb_ca6b)
            .rotate_left((seed ^ 0x13) & 31);
        let right = seed
            .wrapping_mul(0xc2b2_ae35)
            .rotate_right((seed ^ 0x29) & 31);
        let input = binary_input(left, right);

        for (op_id, expected) in [
            ("primitive.arith.add", left.wrapping_add(right)),
            ("primitive.arith.mul", left.wrapping_mul(right)),
        ] {
            let (reference_a, reference_b) =
                resolve_dual(op_id).unwrap_or_else(|| panic!("Fix: {op_id} must be registered."));
            let output_a = reference_a(&input);
            let output_b = reference_b(&input);
            let expected = expected.to_le_bytes().to_vec();

            assert_eq!(
                output_a, output_b,
                "Fix: arithmetic dual references diverged for {op_id} left={left:#010x} right={right:#010x}"
            );
            assert_eq!(
                output_a, expected,
                "Fix: arithmetic oracle returned wrong wrapping result for {op_id} left={left:#010x} right={right:#010x}"
            );
            assertions += 2;
        }
    }
    assert_eq!(assertions, 8192 * 4);
}

#[test]
fn arithmetic_dual_references_treat_malformed_short_input_as_zero_word() {
    for op_id in ["primitive.arith.add", "primitive.arith.mul"] {
        let (reference_a, reference_b) =
            resolve_dual(op_id).unwrap_or_else(|| panic!("Fix: {op_id} must be registered."));
        for input in [&[][..], &[0xFF][..], &[0x01, 0x02, 0x03, 0x04][..]] {
            assert_eq!(
                reference_a(input),
                vec![0; 4],
                "Fix: {op_id} ref_a short-input contract drifted"
            );
            assert_eq!(
                reference_b(input),
                vec![0; 4],
                "Fix: {op_id} ref_b short-input contract drifted"
            );
        }
    }
}
