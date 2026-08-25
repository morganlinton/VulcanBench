//! `bitset_and_not`  -  per-word `lhs AND NOT rhs` over packed bitsets.
//!
//! Produced as a first-class primitive so set-difference (subtract
//! `rhs` from `lhs`) is one Region instead of the two-op compose
//! `bitset_not(rhs)` → `bitset_and(lhs, allow)`. Downstream analyzer's
//! `flows_to_not_via` lowering uses this to subtract waypoint nodes
//! from the source frontier, making the `not_via` path one fewer
//! buffer + one fewer dispatch than the manual compose.

use super::binary_word::{binary_word_program, BitwiseBinaryOp};
use vyre_foundation::ir::Program;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::bitset::and_not";

/// Build a Program: `out[w] = lhs[w] & !rhs[w]`.
///
/// Per-thread per-word implementation. Equivalent CPU oracle:
/// `lhs.iter().zip(rhs).map(|(a,b)| a & !b).collect()`.
#[must_use]
pub fn bitset_and_not(lhs: &str, rhs: &str, out: &str, words: u32) -> Program {
    binary_word_program(OP_ID, lhs, rhs, out, words, BitwiseBinaryOp::AndNot)
}

/// CPU reference: `out[i] = lhs[i] & !rhs[i]` per word.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(lhs: &[u32], rhs: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    match try_cpu_ref_into(lhs, rhs, &mut out) {
        Ok(()) => out,
        // A parity oracle that returns empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use try_cpu_ref_into.
        Err(error) => panic!("vyre-primitives bitset_and_not cpu_ref failed: {error}"),
    }
}

/// CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(lhs: &[u32], rhs: &[u32], out: &mut Vec<u32>) {
    if let Err(error) = try_cpu_ref_into(lhs, rhs, out) {
        panic!("vyre-primitives bitset_and_not cpu_ref_into failed: {error}");
    }
}

/// Fallible CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(lhs: &[u32], rhs: &[u32], out: &mut Vec<u32>) -> Result<(), String> {
    let len = lhs.len().min(rhs.len());
    if len > out.capacity() {
        out.try_reserve(len - out.len()).map_err(|err| {
            format!(
                "bitset_and_not CPU oracle failed to reserve {len} output words: {err}. Fix: shard the bitset before parity evaluation."
            )
        })?;
    }
    out.clear();
    out.extend(lhs.iter().zip(rhs.iter()).map(|(a, b)| a & !b));
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || bitset_and_not("lhs", "rhs", "out", 2),
        Some(|| {
            let to_bytes = |words: &[u32]| crate::wire::pack_u32_slice(words);
            vec![vec![
                to_bytes(&[0xFF00, 0xAAAA_AAAA]),
                to_bytes(&[0xF0F0, 0x5555_5555]),
                to_bytes(&[0, 0]),
            ]]
        }),
        Some(|| {
            let to_bytes = |words: &[u32]| crate::wire::pack_u32_slice(words);
            vec![vec![to_bytes(&[0x0F00, 0xAAAA_AAAA])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_word_and_not() {
        // 0xFF00 with 0xF0F0 removed = 0x0F00.
        assert_eq!(cpu_ref(&[0xFF00], &[0xF0F0]), vec![0x0F00]);
    }

    #[test]
    fn empty_rhs_passes_lhs_through() {
        assert_eq!(cpu_ref(&[0xDEAD_BEEF], &[0]), vec![0xDEAD_BEEF]);
    }

    #[test]
    fn full_rhs_zeros_output() {
        assert_eq!(cpu_ref(&[0xDEAD_BEEF], &[0xFFFF_FFFF]), vec![0]);
    }

    #[test]
    fn distributes_over_multiple_words() {
        let lhs = [0xFFFF_FFFF, 0x0F0F_0F0F, 0xAAAA_AAAA];
        let rhs = [0x0000_FFFF, 0xF0F0_F0F0, 0x5555_5555];
        let want = [0xFFFF_0000, 0x0F0F_0F0F, 0xAAAA_AAAA];
        assert_eq!(cpu_ref(&lhs, &rhs), want);
    }

    // ------------------------------------------------------------------
    // Adversarial fixtures  -  empty, single-word, cross-word, A==B, B=all-1s.
    // ------------------------------------------------------------------

    #[test]
    fn empty_bitset() {
        assert_eq!(cpu_ref(&[], &[]), Vec::<u32>::new());
    }

    #[test]
    fn single_word_all_bits() {
        let lhs = vec![0xFFFF_FFFF];
        let rhs = vec![0x0000_FFFF];
        assert_eq!(cpu_ref(&lhs, &rhs), vec![0xFFFF_0000]);
    }

    #[test]
    fn compatibility_wrappers_match_fallible_reference() {
        let lhs = [0xFFFF_0000, 0x1234_5678];
        let rhs = [0x00FF_00FF, 0xFFFF_0000];
        let mut compat = Vec::with_capacity(4);
        let mut fallible = Vec::with_capacity(4);

        cpu_ref_into(&lhs, &rhs, &mut compat);
        try_cpu_ref_into(&lhs, &rhs, &mut fallible)
            .expect("Fix: small bitset_and_not CPU oracle must reserve");

        assert_eq!(cpu_ref(&lhs, &rhs), fallible);
        assert_eq!(compat, fallible);
    }

    #[test]
    fn production_cpu_ref_wrappers_have_no_raw_panic_path() {
        let production = include_str!("and_not.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: and_not.rs must contain production section");

        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: bitset_and_not CPU parity wrappers must not panic in production."
        );
    }

    #[test]
    fn cross_word_boundary() {
        // Word 0 bit 31 and word 1 bit 0 are adjacent nodes.
        let lhs = vec![0x8000_0000, 0x0000_0001];
        let rhs = vec![0x0000_0000, 0x0000_0000];
        assert_eq!(cpu_ref(&lhs, &rhs), vec![0x8000_0000, 0x0000_0001]);
    }

    #[test]
    fn a_eq_b_produces_all_zeros() {
        let a = vec![0xDEAD_BEEF, 0x0F0F_0F0F];
        assert_eq!(cpu_ref(&a, &a), vec![0, 0]);
    }

    #[test]
    fn b_all_ones_produces_zeros() {
        let lhs = vec![0xFFFF_FFFF, 0xFFFF_FFFF];
        let rhs = vec![0xFFFF_FFFF, 0xFFFF_FFFF];
        assert_eq!(cpu_ref(&lhs, &rhs), vec![0, 0]);
    }

    #[test]
    fn cpu_ref_into_truncates_stale_tail_without_reallocating() {
        let lhs = vec![0xFFFF_FFFF, 0x0F0F_0F0F];
        let rhs = vec![0x0000_FFFF, 0xF0F0_F0F0];
        let mut out = Vec::with_capacity(8);
        out.extend([0xDEAD_BEEF; 8]);
        let ptr = out.as_ptr();

        try_cpu_ref_into(&lhs, &rhs, &mut out).unwrap();

        assert_eq!(out, vec![0xFFFF_0000, 0x0F0F_0F0F]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn generated_and_not_matches_scalar_reference_and_truncates_shorter_input() {
        for lhs_len in 0..64usize {
            let rhs_len = 63usize.saturating_sub(lhs_len / 2);
            let lhs: Vec<u32> = (0..lhs_len)
                .map(|idx| (idx as u32).wrapping_mul(0x9E37_79B9))
                .collect();
            let rhs: Vec<u32> = (0..rhs_len)
                .map(|idx| (idx as u32).wrapping_mul(0x85EB_CA6B).wrapping_add(7))
                .collect();
            let mut out = Vec::with_capacity(lhs_len.min(rhs_len) + 3);

            try_cpu_ref_into(&lhs, &rhs, &mut out).unwrap();

            assert_eq!(
                out,
                lhs.iter()
                    .zip(rhs.iter())
                    .map(|(a, b)| a & !b)
                    .collect::<Vec<_>>(),
                "generated and-not case lhs_len={lhs_len} rhs_len={rhs_len}"
            );
        }
    }
}
