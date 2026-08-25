//! Generated adversarial incremental-hash tests for CRC32, Adler32, and FNV-1a.

use vyre_primitives::hash::adler32::{
    adler32, adler32_finalize_state, adler32_initial_a_state, adler32_initial_b_state,
    adler32_update_byte_state,
};
use vyre_primitives::hash::crc32::{
    build_table, crc32, crc32_finalize_state, crc32_initial_state, crc32_update_byte_state,
};
use vyre_primitives::hash::fnv1a::{
    fnv1a32, fnv1a32_initial_state, fnv1a32_update_byte, fnv1a64, fnv1a64_initial_state,
    fnv1a64_update_byte,
};

fn generated_case(seed: u32) -> Vec<u8> {
    let len = ((seed.wrapping_mul(17) ^ (seed >> 3)) % 257) as usize;
    let mut state = seed ^ 0xA5A5_5A5A;
    let mut bytes = Vec::with_capacity(len);
    for idx in 0..len {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        bytes.push(
            (state
                .wrapping_add(idx as u32)
                .rotate_left((idx % 31) as u32)
                & 0xFF) as u8,
        );
    }
    bytes
}

#[test]
fn incremental_hash_state_helpers_match_slice_hashers_across_generated_adversarial_cases() {
    let crc_table = build_table();
    for seed in 0..4096u32 {
        let bytes = generated_case(seed);

        let mut crc = crc32_initial_state();
        let mut fnv32 = fnv1a32_initial_state();
        let mut fnv64 = fnv1a64_initial_state();
        let mut adler_a = adler32_initial_a_state();
        let mut adler_b = adler32_initial_b_state();

        for &byte in &bytes {
            crc = crc32_update_byte_state(crc, &crc_table, byte);
            fnv32 = fnv1a32_update_byte(fnv32, byte);
            fnv64 = fnv1a64_update_byte(fnv64, byte);
            let adler = adler32_update_byte_state(adler_a, adler_b, byte);
            adler_a = adler.0;
            adler_b = adler.1;
        }

        assert_eq!(
            crc32_finalize_state(crc),
            crc32(&bytes),
            "crc32 seed {seed}"
        );
        assert_eq!(fnv32, fnv1a32(&bytes), "fnv1a32 seed {seed}");
        assert_eq!(fnv64, fnv1a64(&bytes), "fnv1a64 seed {seed}");
        assert_eq!(
            adler32_finalize_state(adler_a, adler_b),
            adler32(&bytes),
            "adler32 seed {seed}"
        );
    }
}

#[test]
fn persistent_bfs_layout_hash_uses_canonical_fnv64_helpers() {
    let source = include_str!("../src/graph/persistent_bfs/hash.rs");
    assert!(
        source.contains("fnv1a64_initial_state") && source.contains("fnv1a64_update_byte"),
        "Fix: persistent BFS layout hashing must use canonical FNV64 helpers."
    );
    assert!(
        !source.contains("0xcbf2_9ce4_8422_2325") && !source.contains("0x0000_0100_0000_01b3"),
        "Fix: persistent BFS layout hashing must not redefine FNV64 constants."
    );
}

#[test]
fn dfa_and_perfect_hash_fingerprints_use_canonical_fnv64_helpers() {
    let nfa_to_dfa = include_str!("../src/matching/nfa_to_dfa.rs");
    let perfect_hash = include_str!("../../vyre-libs/src/intern/perfect_hash.rs");

    for (name, source) in [("nfa_to_dfa", nfa_to_dfa), ("perfect_hash", perfect_hash)] {
        assert!(
            source.contains("fnv1a64_initial_state") && source.contains("fnv1a64_update_byte"),
            "Fix: {name} fingerprinting must use canonical FNV64 helpers."
        );
        assert!(
            !source.contains("0xcbf2_9ce4_8422_2325") && !source.contains("0x0000_0100_0000_01b3"),
            "Fix: {name} must not redefine FNV64 constants."
        );
    }
}

#[test]
fn packed_ast_structural_cse_uses_canonical_fnv32_structural_mix() {
    let ast_cse = include_str!("../src/parsing/ast_cse_structural_hash.rs");
    assert!(
        ast_cse.contains("fnv1a32_mul_xor_word_expr")
            && ast_cse.contains("fnv1a32_mul_xor_word_state"),
        "Fix: packed-AST CSE structural hashing must use canonical FNV-prime structural mix helpers."
    );
    assert!(
        !ast_cse.contains("0x01000193") && !ast_cse.contains("0x0100_0193"),
        "Fix: packed-AST CSE must not inline the FNV32 prime."
    );
}

#[test]
fn fnv_property_tests_use_incremental_helpers_instead_of_redefining_constants() {
    let proptest = include_str!("proptest_hash_fnv1a.rs");
    assert!(
        proptest.contains("fnv1a32_initial_state") && proptest.contains("fnv1a32_update_byte"),
        "Fix: FNV property tests must exercise the canonical incremental helper API."
    );
    assert!(
        !proptest.contains("0x811c_9dc5") && !proptest.contains("0x0100_0193"),
        "Fix: FNV property tests must not carry shadow constants."
    );
}
