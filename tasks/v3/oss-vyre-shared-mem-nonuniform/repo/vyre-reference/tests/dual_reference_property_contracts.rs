//! Generated differential coverage for registered dual references.

use proptest::prelude::*;
use vyre_reference::resolve_dual;

fn binary_input(left: u32, right: u32) -> Vec<u8> {
    let mut input = Vec::with_capacity(8);
    input.extend_from_slice(&left.to_le_bytes());
    input.extend_from_slice(&right.to_le_bytes());
    input
}

proptest! {
    #[test]
    fn generated_bitwise_dual_references_agree(left in any::<u32>(), right in any::<u32>()) {
        for op_id in [
            "primitive.bitwise.xor",
            "primitive.bitwise.and",
            "primitive.bitwise.or",
        ] {
            let (reference_a, reference_b) = resolve_dual(op_id)
                .unwrap_or_else(|| panic!("Fix: {op_id} must have two registered references"));
            let input = binary_input(left, right);

            prop_assert_eq!(
                reference_a(&input),
                reference_b(&input),
                "Fix: dual references diverged for {} on left={:#x}, right={:#x}",
                op_id,
                left,
                right
            );
        }
    }

    #[test]
    fn generated_compare_dual_references_match_boolean_contract(left in any::<u32>(), right in any::<u32>()) {
        for (op_id, expected) in [
            ("primitive.compare.eq", left == right),
            ("primitive.compare.lt", left < right),
        ] {
            let (reference_a, reference_b) = resolve_dual(op_id)
                .unwrap_or_else(|| panic!("Fix: {op_id} must have two registered references"));
            let input = binary_input(left, right);
            let expected_bytes = u32::from(expected).to_le_bytes().to_vec();

            prop_assert_eq!(
                reference_a(&input),
                reference_b(&input),
                "Fix: compare dual references diverged for {} on left={:#x}, right={:#x}",
                op_id,
                left,
                right
            );
            prop_assert_eq!(
                reference_a(&input),
                expected_bytes,
                "Fix: compare dual reference returned the wrong boolean word for {} on left={:#x}, right={:#x}",
                op_id,
                left,
                right
            );
        }
    }

    #[test]
    fn generated_unary_dual_references_agree(value in any::<u32>()) {
        let (reference_a, reference_b) = resolve_dual("primitive.bitwise.not")
            .expect("Fix: primitive.bitwise.not must have two registered references");
        let input = value.to_le_bytes().to_vec();

        prop_assert_eq!(
            reference_a(&input),
            reference_b(&input),
            "Fix: dual not references diverged for value={:#x}",
            value
        );
    }
}
