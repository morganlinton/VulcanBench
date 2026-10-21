//! Generated dual scalar evaluator matrix coverage.

use vyre_primitives::{
    ArithAdd, ArithMul, Clz, CompareEq, CompareLt, Popcount, ShiftLeft, ShiftRight,
};
use vyre_reference::{dual_impls::ReferenceEvaluator, workgroup::Memory};

fn mem(value: u32) -> Memory {
    Memory::from_bytes(value.to_le_bytes().to_vec())
}

fn bad_mem() -> Memory {
    Memory::from_bytes(vec![1, 2, 3])
}

fn word(memory: Memory) -> u32 {
    let bytes = memory.bytes();
    assert_eq!(
        bytes.len(),
        4,
        "Fix: scalar evaluator outputs must be exactly one u32."
    );
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn scalar_values() -> Vec<u32> {
    let mut values = vec![
        0,
        1,
        2,
        3,
        7,
        8,
        15,
        16,
        31,
        32,
        63,
        64,
        127,
        128,
        255,
        256,
        1023,
        1024,
        u16::MAX as u32,
        (u16::MAX as u32) + 1,
        i32::MAX as u32,
        i32::MIN as u32,
        u32::MAX - 1,
        u32::MAX,
    ];
    let mut x = 0x9e37_79b9u32;
    for _ in 0..232 {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        values.push(x);
    }
    values.sort_unstable();
    values.dedup();
    values
}

#[test]
fn generated_binary_scalar_evaluators_match_u32_contract_matrix() {
    let values = scalar_values();
    for &left in &values {
        for &right in &values {
            let inputs = [mem(left), mem(right)];
            assert_eq!(
                word(
                    ArithAdd
                        .evaluate(&inputs)
                        .expect("Fix: arith_add must accept two u32 payloads.")
                ),
                left.wrapping_add(right)
            );
            assert_eq!(
                word(
                    ArithMul
                        .evaluate(&inputs)
                        .expect("Fix: arith_mul must accept two u32 payloads.")
                ),
                left.wrapping_mul(right)
            );
            assert_eq!(
                word(
                    CompareEq
                        .evaluate(&inputs)
                        .expect("Fix: compare_eq must accept two u32 payloads.")
                ),
                u32::from(left == right)
            );
            assert_eq!(
                word(
                    CompareLt
                        .evaluate(&inputs)
                        .expect("Fix: compare_lt must accept two u32 payloads.")
                ),
                u32::from(left < right)
            );
            assert_eq!(
                word(
                    ShiftLeft
                        .evaluate(&inputs)
                        .expect("Fix: shift_left must accept two u32 payloads.")
                ),
                left << (right & 31)
            );
            assert_eq!(
                word(
                    ShiftRight
                        .evaluate(&inputs)
                        .expect("Fix: shift_right must accept two u32 payloads.")
                ),
                left >> (right & 31)
            );
        }
    }
}

#[test]
fn generated_unary_scalar_evaluators_match_u32_contract_matrix() {
    for value in scalar_values() {
        let inputs = [mem(value)];
        assert_eq!(
            word(
                Clz.evaluate(&inputs)
                    .expect("Fix: clz must accept one u32 payload.")
            ),
            value.leading_zeros()
        );
        assert_eq!(
            word(
                Popcount
                    .evaluate(&inputs)
                    .expect("Fix: popcount must accept one u32 payload.")
            ),
            value.count_ones()
        );
    }
}

#[test]
fn scalar_evaluators_reject_wrong_arity_and_unaligned_payloads() {
    assert!(matches!(ArithAdd.evaluate(&[mem(1)]), Err(_)));
    assert!(matches!(ArithAdd.evaluate(&[bad_mem(), mem(1)]), Err(_)));
    assert!(matches!(ArithMul.evaluate(&[mem(1)]), Err(_)));
    assert!(matches!(ArithMul.evaluate(&[mem(1), bad_mem()]), Err(_)));
    assert!(matches!(CompareEq.evaluate(&[mem(1)]), Err(_)));
    assert!(matches!(CompareEq.evaluate(&[bad_mem(), mem(1)]), Err(_)));
    assert!(matches!(CompareLt.evaluate(&[mem(1)]), Err(_)));
    assert!(matches!(CompareLt.evaluate(&[mem(1), bad_mem()]), Err(_)));
    assert!(matches!(ShiftLeft.evaluate(&[mem(1)]), Err(_)));
    assert!(matches!(ShiftLeft.evaluate(&[bad_mem(), mem(1)]), Err(_)));
    assert!(matches!(ShiftRight.evaluate(&[mem(1)]), Err(_)));
    assert!(matches!(ShiftRight.evaluate(&[mem(1), bad_mem()]), Err(_)));
    assert!(matches!(Clz.evaluate(&[]), Err(_)));
    assert!(matches!(Clz.evaluate(&[bad_mem()]), Err(_)));
    assert!(matches!(Popcount.evaluate(&[]), Err(_)));
    assert!(matches!(Popcount.evaluate(&[bad_mem()]), Err(_)));
}
