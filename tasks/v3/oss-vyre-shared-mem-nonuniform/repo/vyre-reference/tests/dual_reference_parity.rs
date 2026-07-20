//! Dual-reference parity tests  -  verify both independently-written
//! references agree on a range of inputs for every registered op.

use vyre_reference::{dual_op_ids, resolve_dual};

/// Generate test vectors that exercise edge cases for binary u32 ops.
fn binary_word_test_vectors() -> Vec<Vec<u8>> {
    let pairs: Vec<(u32, u32)> = vec![
        (0, 0),
        (0xFFFF_FFFF, 0),
        (0, 0xFFFF_FFFF),
        (0xFFFF_FFFF, 0xFFFF_FFFF),
        (0xAAAA_AAAA, 0x5555_5555),
        (1, 1),
        (0xDEAD_BEEF, 0xCAFE_BABE),
        (0x8000_0000, 0x7FFF_FFFF),
        (42, 0),
        (0x1234_5678, 0x9ABC_DEF0),
    ];
    pairs
        .into_iter()
        .map(|(a, b)| {
            let mut v = Vec::with_capacity(8);
            v.extend_from_slice(&a.to_le_bytes());
            v.extend_from_slice(&b.to_le_bytes());
            v
        })
        .collect()
}

/// Generate single-operand test vectors for unary ops (e.g. NOT).
fn unary_test_vectors() -> Vec<Vec<u8>> {
    let values: Vec<u32> = vec![
        0,
        1,
        0xFFFF_FFFF,
        0xAAAA_AAAA,
        0x5555_5555,
        0xDEAD_BEEF,
        0x8000_0000,
        42,
        0x1234_5678,
        0x7FFF_FFFF,
    ];
    values
        .into_iter()
        .map(|v| v.to_le_bytes().to_vec())
        .collect()
}

#[test]
fn all_dual_ops_are_resolvable() {
    for op_id in dual_op_ids() {
        assert!(
            resolve_dual(op_id).is_some(),
            "Fix: dual-reference for op '{op_id}' is listed in dual_op_ids() but not resolvable via resolve_dual(). Register both references."
        );
    }
}

#[test]
fn dual_references_agree_on_binary_word_ops() {
    let binary_ops: Vec<&str> = dual_op_ids()
        .iter()
        .copied()
        .filter(|id| {
            *id == "primitive.bitwise.xor"
                || *id == "primitive.bitwise.and"
                || *id == "primitive.bitwise.or"
                || *id == "primitive.bitwise.shift_left"
                || *id == "primitive.bitwise.shift_right"
                || *id == "primitive.compare.eq"
                || *id == "primitive.compare.lt"
        })
        .collect();

    let vectors = binary_word_test_vectors();

    for op_id in binary_ops {
        let (ref_a, ref_b) = resolve_dual(op_id)
            .unwrap_or_else(|| panic!("Fix: resolve_dual({op_id}) must succeed."));

        for (i, input) in vectors.iter().enumerate() {
            let out_a = ref_a(input);
            let out_b = ref_b(input);
            assert_eq!(
                out_a, out_b,
                "Fix: dual references for '{op_id}' diverged on vector {i}: input={input:?}, ref_a={out_a:?}, ref_b={out_b:?}"
            );
        }
    }
}

#[test]
fn dual_references_agree_on_unary_bitwise_ops() {
    let unary_ops: Vec<&str> = dual_op_ids()
        .iter()
        .copied()
        .filter(|id| {
            *id == "primitive.bitwise.not"
                || *id == "primitive.bitwise.popcount"
                || *id == "primitive.bitwise.clz"
        })
        .collect();

    let vectors = unary_test_vectors();

    for op_id in unary_ops {
        let (ref_a, ref_b) = resolve_dual(op_id)
            .unwrap_or_else(|| panic!("Fix: resolve_dual({op_id}) must succeed."));

        for (i, input) in vectors.iter().enumerate() {
            let out_a = ref_a(input);
            let out_b = ref_b(input);
            assert_eq!(
                out_a, out_b,
                "Fix: dual references for '{op_id}' diverged on vector {i}: input={input:?}, ref_a={out_a:?}, ref_b={out_b:?}"
            );
        }
    }
}

#[test]
fn dual_op_count_is_at_least_four() {
    assert!(
        dual_op_ids().len() >= 10,
        "Fix: dual_op_ids() must enumerate at least 10 ops for meaningful differential coverage; currently has {}.",
        dual_op_ids().len()
    );
}
