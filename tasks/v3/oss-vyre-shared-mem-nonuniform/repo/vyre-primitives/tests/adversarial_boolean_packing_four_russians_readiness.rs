//! Adversarial contract tests for boolean packing substrate and
//! Four-Russians readiness contracts.
#![cfg(feature = "bitset")]

mod common;
use common::u32_bytes;

// Coverage: bitset_words sizing, word-aligned packing invariants,
// set-relation predicates (equal, subset_of, contains, test_bit),
// elementwise ops (and, or, xor, not), popcount, and boundary
// element counts that stress cross-word behaviour.
//
// GPU acquisition: none  -  all assertions use CPU reference oracles.

use vyre_primitives::bitset::and::cpu_ref as bitset_and_ref;
use vyre_primitives::bitset::and_not::cpu_ref as bitset_and_not_ref;
use vyre_primitives::bitset::any::cpu_ref as bitset_any_ref;
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::bitset::contains::cpu_ref as bitset_contains_ref;
use vyre_primitives::bitset::equal::cpu_ref as bitset_equal_ref;
use vyre_primitives::bitset::four_russians::{
    binary_byte_lut, cached_binary_byte_lut, cpu_ref as four_russians_cpu_ref,
    four_russians_apply_byte_lut, BooleanTileOp,
};
use vyre_primitives::bitset::not::cpu_ref as bitset_not_ref;
use vyre_primitives::bitset::or::cpu_ref as bitset_or_ref;
use vyre_primitives::bitset::popcount::cpu_ref as bitset_popcount_ref;
use vyre_primitives::bitset::subset_of::cpu_ref as bitset_subset_of_ref;
use vyre_primitives::bitset::test_bit::cpu_ref as bitset_test_bit_ref;
use vyre_primitives::bitset::xor::cpu_ref as bitset_xor_ref;
use vyre_reference::value::Value;

// ---------------------------------------------------------------------------
// Boolean packing invariants: ops must be correct on partial final words
// ---------------------------------------------------------------------------

fn pack_bits(node_count: u32, set_bits: &[u32]) -> Vec<u32> {
    let words = bitset_words(node_count) as usize;
    let mut buf = vec![0u32; words];
    for &b in set_bits {
        let w = (b / 32) as usize;
        if w < buf.len() {
            buf[w] |= 1 << (b % 32);
        }
    }
    buf
}

#[test]
fn and_preserves_partial_word_bits() {
    // 33 elements: 2 words. Word 1 only has bit 0 valid (element 32).
    let lhs = pack_bits(33, &[0, 32]);
    let rhs = pack_bits(33, &[0, 1, 32]);
    let got = bitset_and_ref(&lhs, &rhs);
    assert_eq!(got, pack_bits(33, &[0, 32]));
}

#[test]
fn or_preserves_partial_word_bits() {
    let lhs = pack_bits(33, &[0]);
    let rhs = pack_bits(33, &[32]);
    let got = bitset_or_ref(&lhs, &rhs);
    assert_eq!(got, pack_bits(33, &[0, 32]));
}

#[test]
fn xor_preserves_partial_word_bits() {
    let lhs = pack_bits(33, &[0, 32]);
    let rhs = pack_bits(33, &[0]);
    let got = bitset_xor_ref(&lhs, &rhs);
    assert_eq!(got, pack_bits(33, &[32]));
}

#[test]
fn not_inverts_all_allocated_words() {
    let input = pack_bits(33, &[0]);
    let got = bitset_not_ref(&input);
    // Word 0: all bits except 0 should be set
    assert_eq!(got[0], !1u32);
    // Word 1: bitset_not_ref inverts the ENTIRE word, not just valid bits.
    // Callers that need partial-word masking must AND with a validity mask.
    assert_eq!(got[1], 0xFFFF_FFFF);
}

#[test]
fn and_not_on_partial_word() {
    let lhs = pack_bits(33, &[0, 32]);
    let rhs = pack_bits(33, &[0]);
    let got = bitset_and_not_ref(&lhs, &rhs);
    assert_eq!(got, pack_bits(33, &[32]));
}

// ---------------------------------------------------------------------------
// Set-relation predicates
// ---------------------------------------------------------------------------

#[test]
fn equal_empty_bitsets() {
    assert_eq!(bitset_equal_ref(&[], &[]), 1);
}

#[test]
fn equal_identical_non_trivial() {
    let a = pack_bits(100, &[0, 31, 32, 63, 64, 99]);
    let b = pack_bits(100, &[0, 31, 32, 63, 64, 99]);
    assert_eq!(bitset_equal_ref(&a, &b), 1);
}

#[test]
fn equal_differs_in_last_word() {
    let a = pack_bits(33, &[32]);
    let b = pack_bits(33, &[]);
    assert_eq!(bitset_equal_ref(&a, &b), 0);
}

#[test]
fn equal_length_mismatch() {
    assert_eq!(bitset_equal_ref(&[0; 2], &[0; 3]), 0);
}

#[test]
fn subset_of_proper_subset() {
    let a = pack_bits(64, &[0, 1, 2]);
    let b = pack_bits(64, &[0, 1, 2, 3]);
    assert_eq!(bitset_subset_of_ref(&a, &b), 1);
}

#[test]
fn subset_of_equal_sets() {
    let a = pack_bits(64, &[0, 31, 32, 63]);
    let b = pack_bits(64, &[0, 31, 32, 63]);
    assert_eq!(bitset_subset_of_ref(&a, &b), 1);
}

#[test]
fn subset_of_not_subset() {
    let a = pack_bits(64, &[0, 1]);
    let b = pack_bits(64, &[1, 2]);
    assert_eq!(bitset_subset_of_ref(&a, &b), 0);
}

#[test]
fn subset_of_empty_is_subset_of_anything() {
    let empty = pack_bits(64, &[]);
    let full = vec![0xFFFF_FFFFu32; 2];
    assert_eq!(bitset_subset_of_ref(&empty, &full), 1);
}

#[test]
fn subset_of_anything_is_not_subset_of_empty_unless_empty() {
    let a = pack_bits(64, &[0]);
    let empty = pack_bits(64, &[]);
    assert_eq!(bitset_subset_of_ref(&a, &empty), 0);
}

#[test]
fn subset_of_lhs_longer_than_rhs() {
    let a = vec![0u32, 1u32]; // 64 bits, but second word has bit 0 set
    let b = vec![0u32]; // only 32 bits
    assert_eq!(bitset_subset_of_ref(&a, &b), 0);
}

#[test]
fn subset_of_rhs_longer_than_lhs_all_zero_tail() {
    let a = vec![0u32];
    let b = vec![0u32, 0u32];
    assert_eq!(bitset_subset_of_ref(&a, &b), 1);
}

// ---------------------------------------------------------------------------
// Point queries
// ---------------------------------------------------------------------------

#[test]
fn contains_first_and_last_bits() {
    let bits = pack_bits(128, &[0, 127]);
    assert_eq!(bitset_contains_ref(&bits, 0), 1);
    assert_eq!(bitset_contains_ref(&bits, 127), 1);
    assert_eq!(bitset_contains_ref(&bits, 1), 0);
    assert_eq!(bitset_contains_ref(&bits, 126), 0);
}

#[test]
fn contains_cross_word_boundaries() {
    let bits = pack_bits(128, &[31, 32, 63, 64]);
    assert_eq!(bitset_contains_ref(&bits, 31), 1);
    assert_eq!(bitset_contains_ref(&bits, 32), 1);
    assert_eq!(bitset_contains_ref(&bits, 63), 1);
    assert_eq!(bitset_contains_ref(&bits, 64), 1);
}

#[test]
fn contains_out_of_bounds_returns_zero() {
    let bits = pack_bits(33, &[0]);
    assert_eq!(bitset_contains_ref(&bits, 33), 0);
    assert_eq!(bitset_contains_ref(&bits, 1024), 0);
}

#[test]
fn test_bit_first_and_last() {
    let bits = pack_bits(128, &[0, 127]);
    assert_eq!(bitset_test_bit_ref(&bits, 0), 1);
    assert_eq!(bitset_test_bit_ref(&bits, 127), 1);
    assert_eq!(bitset_test_bit_ref(&bits, 1), 0);
}

#[test]
fn test_bit_out_of_bounds_returns_zero() {
    let bits = vec![0xFFFF_FFFFu32];
    assert_eq!(bitset_test_bit_ref(&bits, 32), 0);
    assert_eq!(bitset_test_bit_ref(&bits, 1024), 0);
}

// ---------------------------------------------------------------------------
// Popcount
// ---------------------------------------------------------------------------

#[test]
fn popcount_empty() {
    assert_eq!(bitset_popcount_ref(&[]), Vec::<u32>::new());
}

#[test]
fn popcount_all_ones() {
    let input = vec![0xFFFF_FFFFu32; 4];
    assert_eq!(bitset_popcount_ref(&input), vec![32; 4]);
}

#[test]
fn popcount_mixed_words() {
    let input = vec![0b1010u32, 0xFF00_FF00, 0x0000_0001];
    assert_eq!(bitset_popcount_ref(&input), vec![2, 16, 1]);
}

#[test]
fn popcount_partial_word_semantic() {
    // Even though only 33 elements exist, popcount counts ALL bits in the word.
    // This is the correct per-word primitive; total bitset popcount needs a reduction.
    let input = pack_bits(33, &[0, 32]);
    assert_eq!(bitset_popcount_ref(&input), vec![1, 1]);
}

// ---------------------------------------------------------------------------
// any
// ---------------------------------------------------------------------------

#[test]
fn any_empty_is_zero() {
    assert_eq!(bitset_any_ref(&[]), 0);
}

#[test]
fn any_all_zeros_is_zero() {
    assert_eq!(bitset_any_ref(&[0, 0, 0]), 0);
}

#[test]
fn any_single_bit_set() {
    assert_eq!(bitset_any_ref(&[0, 0x8000_0000, 0]), 1);
}

#[test]
fn any_partial_word_only() {
    let bits = pack_bits(33, &[32]);
    assert_eq!(bitset_any_ref(&bits), 1);
}

// ---------------------------------------------------------------------------
// Four-Russians readiness contracts
// ---------------------------------------------------------------------------

#[test]
fn four_russians_word_alignment_invariant() {
    // Four-Russians speedups require that the boolean matrix / bitvector
    // is packed into machine words so that lookup tables can be indexed
    // by whole words. bitset_words must always round up to the next word.
    for n in [1, 7, 8, 15, 16, 31, 32, 33, 63, 64, 127, 128, 255, 256] {
        let words = bitset_words(n);
        assert!(
            words * 32 >= n,
            "bitset_words({n}) must cover at least {n} bits"
        );
        assert!(
            (words.saturating_sub(1)) * 32 < n || words == 0,
            "bitset_words({n}) must not over-allocate by a whole word"
        );
    }
}

#[test]
fn four_russians_tileable_word_ops() {
    // A prerequisite for Four-Russians tiling is that elementwise ops
    // over equal-length bitsets are correct for any word-aligned length.
    let lengths = [1, 2, 3, 4, 7, 8];
    for words in lengths {
        let a: Vec<u32> = (0..words)
            .map(|i: u32| i.wrapping_mul(0x9E3779B9))
            .collect();
        let b: Vec<u32> = (0..words)
            .map(|i: u32| i.wrapping_mul(0x85EBCA77))
            .collect();

        let and_got = bitset_and_ref(&a, &b);
        let or_got = bitset_or_ref(&a, &b);
        let xor_got = bitset_xor_ref(&a, &b);
        let not_got = bitset_not_ref(&a);

        for i in 0..words {
            let idx = i as usize;
            assert_eq!(
                and_got[idx],
                a[idx] & b[idx],
                "AND tile mismatch at word {i}"
            );
            assert_eq!(or_got[idx], a[idx] | b[idx], "OR tile mismatch at word {i}");
            assert_eq!(
                xor_got[idx],
                a[idx] ^ b[idx],
                "XOR tile mismatch at word {i}"
            );
            assert_eq!(not_got[idx], !a[idx], "NOT tile mismatch at word {i}");
        }
    }
}

#[test]
fn four_russians_subset_is_constant_time_per_word() {
    // The subset_of primitive visits each word once and exits early on
    // the first violation. For Four-Russians readiness we verify that
    // the CPU reference correctly models the early-exit semantics.
    let a = vec![0xFFFF_FFFFu32; 64];
    let b = vec![0xFFFF_FFFFu32; 64];
    assert_eq!(bitset_subset_of_ref(&a, &b), 1);

    let mut c = b.clone();
    c[31] = 0; // violation in the middle
    assert_eq!(bitset_subset_of_ref(&a, &c), 0);
}

#[test]
fn four_russians_binary_luts_match_boolean_ops() {
    let lhs = [0xFF00_FF00u32, 0x0F0F_0F0F, 0xAAAA_5555];
    let rhs = [0xF0F0_F0F0u32, 0xFFFF_0000, 0x3333_CCCC];

    let and_lut = binary_byte_lut(BooleanTileOp::And);
    let or_lut = binary_byte_lut(BooleanTileOp::Or);
    let xor_lut = binary_byte_lut(BooleanTileOp::Xor);
    let and_not_lut = binary_byte_lut(BooleanTileOp::AndNot);

    assert_eq!(
        four_russians_cpu_ref(&lhs, &rhs, &and_lut),
        bitset_and_ref(&lhs, &rhs)
    );
    assert_eq!(
        four_russians_cpu_ref(&lhs, &rhs, &or_lut),
        bitset_or_ref(&lhs, &rhs)
    );
    assert_eq!(
        four_russians_cpu_ref(&lhs, &rhs, &xor_lut),
        bitset_xor_ref(&lhs, &rhs)
    );
    assert_eq!(
        four_russians_cpu_ref(&lhs, &rhs, &and_not_lut),
        bitset_and_not_ref(&lhs, &rhs)
    );
}

#[test]
fn four_russians_cached_luts_reuse_allocation_and_match_owned_tables() {
    let owned = binary_byte_lut(BooleanTileOp::And);
    let cached_first = cached_binary_byte_lut(BooleanTileOp::And);
    let cached_second = cached_binary_byte_lut(BooleanTileOp::And);

    assert_eq!(cached_first, owned.as_slice());
    assert_eq!(
        cached_first.as_ptr(),
        cached_second.as_ptr(),
        "standard Four-Russians LUTs must be process-cached instead of rebuilt per batch"
    );
}

#[test]
fn four_russians_ir_apply_lut_matches_cpu_reference() {
    let lhs = [0xFF00_FF00u32, 0x0F0F_0F0F];
    let rhs = [0xF0F0_F0F0u32, 0xFFFF_0000];
    let lut = binary_byte_lut(BooleanTileOp::And);
    let program = four_russians_apply_byte_lut("lhs", "rhs", "lut", "out", lhs.len() as u32);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&lhs)),
            Value::from(u32_bytes(&rhs)),
            Value::from(u32_bytes(&lut)),
            Value::from(vec![0u8; lhs.len() * 4]),
        ],
    )
    .expect("four_russians_apply_byte_lut must execute");

    assert_eq!(
        outputs[0].to_bytes(),
        u32_bytes(&four_russians_cpu_ref(&lhs, &rhs, &lut))
    );
}

// ---------------------------------------------------------------------------
// Adversarial boundary cases
// ---------------------------------------------------------------------------

#[test]
fn all_ops_on_maximum_word_count() {
    // 128 bits = 4 words. Stress the widest common case.
    let a = vec![0x1234_5678u32, 0x9ABC_DEF0, 0x0F0F_0F0F, 0xF0F0_F0F0];
    let b = vec![0xF0F0_F0F0u32, 0x0F0F_0F0F, 0x9ABC_DEF0, 0x1234_5678];

    let and_got = bitset_and_ref(&a, &b);
    let or_got = bitset_or_ref(&a, &b);
    let xor_got = bitset_xor_ref(&a, &b);

    for i in 0..4 {
        assert_eq!(and_got[i], a[i] & b[i]);
        assert_eq!(or_got[i], a[i] | b[i]);
        assert_eq!(xor_got[i], a[i] ^ b[i]);
    }
}

#[test]
fn not_is_involution() {
    let a = vec![0xA5A5_A5A5u32, 0x5A5A_5A5A];
    let not_not_a = bitset_not_ref(&bitset_not_ref(&a));
    assert_eq!(not_not_a, a, "NOT(NOT(x)) must equal x");
}

#[test]
fn xor_is_self_inverse() {
    let a = vec![0xDEAD_BEEFu32];
    let b = vec![0xDEAD_BEEFu32];
    assert_eq!(bitset_xor_ref(&a, &b), vec![0], "x ^ x must be 0");
}

#[test]
fn or_is_idempotent() {
    let a = vec![0xCAFE_BABEu32];
    assert_eq!(
        bitset_or_ref(&a, &a),
        vec![0xCAFE_BABEu32],
        "x | x must equal x"
    );
}

#[test]
fn and_is_idempotent() {
    let a = vec![0xCAFE_BABEu32];
    assert_eq!(
        bitset_and_ref(&a, &a),
        vec![0xCAFE_BABEu32],
        "x & x must equal x"
    );
}

#[test]
fn de_morgan_law_holds_at_word_level() {
    let a = vec![0xF0F0_F0F0u32];
    let b = vec![0x0F0F_0F0Fu32];
    // NOT(a AND b) == NOT(a) OR NOT(b)
    let lhs = bitset_not_ref(&bitset_and_ref(&a, &b));
    let rhs = bitset_or_ref(&bitset_not_ref(&a), &bitset_not_ref(&b));
    assert_eq!(lhs, rhs, "De Morgan's law must hold for word-level bitsets");
}
