//! Handwritten oracle matrix for binary/unary bitset maps.
//!
//! Compares production `cpu_ref` / `cpu_ref_into` against independent element-wise
//! oracles on 2048 generated binary pairs (xor/and/or) plus unary not/popcount.

#![forbid(unsafe_code)]
#![cfg(feature = "cpu-parity")]

type UnaryVector = fn(&[u32]) -> Vec<u32>;
type UnaryVectorInto = fn(&[u32], &mut Vec<u32>);
type BinaryVector = fn(&[u32], &[u32]) -> Vec<u32>;
type BinaryVectorInto = fn(&[u32], &[u32], &mut Vec<u32>);

#[test]
fn binary_bitset_maps_match_independent_oracle_matrix() {
    assert_binary(
        "bitset_xor",
        vyre_primitives::bitset::xor::cpu_ref,
        vyre_primitives::bitset::xor::cpu_ref_into,
        |lhs, rhs| {
            lhs.iter()
                .zip(rhs)
                .map(|(left, right)| left ^ right)
                .collect()
        },
    );
    assert_binary(
        "bitset_and",
        vyre_primitives::bitset::and::cpu_ref,
        vyre_primitives::bitset::and::cpu_ref_into,
        |lhs, rhs| {
            lhs.iter()
                .zip(rhs)
                .map(|(left, right)| left & right)
                .collect()
        },
    );
    assert_binary(
        "bitset_or",
        vyre_primitives::bitset::or::cpu_ref,
        vyre_primitives::bitset::or::cpu_ref_into,
        |lhs, rhs| {
            lhs.iter()
                .zip(rhs)
                .map(|(left, right)| left | right)
                .collect()
        },
    );
}

#[test]
fn unary_bitset_maps_match_independent_oracle_matrix() {
    assert_unary(
        "bitset_not",
        vyre_primitives::bitset::not::cpu_ref,
        vyre_primitives::bitset::not::cpu_ref_into,
        |input| input.iter().map(|word| !word).collect(),
    );
    assert_unary(
        "bitset_popcount",
        vyre_primitives::bitset::popcount::cpu_ref,
        vyre_primitives::bitset::popcount::cpu_ref_into,
        |input| input.iter().map(|word| word.count_ones()).collect(),
    );
}

fn assert_binary(
    name: &str,
    actual: BinaryVector,
    actual_into: BinaryVectorInto,
    expected: BinaryVector,
) {
    for (case_idx, (lhs, rhs)) in binary_pairs().enumerate() {
        let expected_out = expected(&lhs, &rhs);
        assert_eq!(
            actual(&lhs, &rhs),
            expected_out,
            "Fix: {name} binary oracle case {case_idx} lhs_len={} rhs_len={} must match the independent oracle.",
            lhs.len(),
            rhs.len()
        );

        let mut reused = vec![0x5A5A_5A5A; lhs.len().max(rhs.len()).saturating_add(11)];
        actual_into(&lhs, &rhs, &mut reused);
        assert_eq!(
            reused, expected_out,
            "Fix: {name} cpu_ref_into binary oracle case {case_idx} must clear stale output capacity before writing."
        );
    }
}

fn assert_unary(
    name: &str,
    actual: UnaryVector,
    actual_into: UnaryVectorInto,
    expected: UnaryVector,
) {
    for (case_idx, input) in binary_pairs().map(|(lhs, _)| lhs).enumerate() {
        let expected_out = expected(&input);
        assert_eq!(
            actual(&input),
            expected_out,
            "Fix: {name} unary oracle case {case_idx} len={} must match the independent oracle.",
            input.len()
        );

        let mut reused = vec![0xA5A5_A5A5; input.len().saturating_add(9)];
        actual_into(&input, &mut reused);
        assert_eq!(
            reused, expected_out,
            "Fix: {name} cpu_ref_into unary oracle case {case_idx} must clear stale output capacity before writing."
        );
    }
}

fn binary_pairs() -> impl Iterator<Item = (Vec<u32>, Vec<u32>)> {
    (0..16384usize).map(|case| {
        let seed = case as u64 ^ 0xB175_E7B1_7500_0000;
        let lhs_len = 1 + ((seed >> 3) as usize % 129);
        let rhs_len = 1 + ((seed >> 11) as usize % 129);
        let lhs = lcg_u64(seed, lhs_len);
        let rhs = lcg_u64(seed.rotate_left(17) ^ 0xD00D_F00D, rhs_len);
        (lhs, rhs)
    })
}

fn lcg_u64(seed: u64, len: usize) -> Vec<u32> {
    let mut state = seed;
    (0..len)
        .map(|idx| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            (state as u32)
                .rotate_left((idx % 31) as u32)
                .wrapping_mul(0x9E37_79B9)
        })
        .collect()
}
