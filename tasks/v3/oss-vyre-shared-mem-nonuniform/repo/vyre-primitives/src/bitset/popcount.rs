//! `bitset_popcount`  -  per-word population count over a packed bitset.
//!
//! Produces a parallel `count_words[w]` array whose sum reduction
//! yields the total bit count. Reductions to a single scalar live
//! under [`crate::reduce`]; this primitive handles just the per-word
//! popcount so it can be composed.

use vyre_foundation::ir::{Program, UnOp};

use crate::bitset::unary_word::bitset_unary_word_program;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::bitset::popcount";

/// Build a Program: `count_words[w] = popcount(input[w])`.
#[must_use]
pub fn bitset_popcount(input: &str, count_words: &str, words: u32) -> Program {
    bitset_unary_word_program(OP_ID, input, count_words, words, UnOp::Popcount)
}

/// CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(input: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    match try_cpu_ref_into(input, &mut out) {
        Ok(()) => out,
        // A parity oracle that returns empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use try_cpu_ref_into.
        Err(error) => panic!("vyre-primitives bitset_popcount cpu_ref failed: {error}"),
    }
}

/// CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(input: &[u32], out: &mut Vec<u32>) {
    if let Err(error) = try_cpu_ref_into(input, out) {
        panic!("vyre-primitives bitset_popcount cpu_ref_into failed: {error}");
    }
}

/// Fallible CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(input: &[u32], out: &mut Vec<u32>) -> Result<(), String> {
    if input.len() > out.capacity() {
        out.try_reserve(input.len() - out.len()).map_err(|err| {
            format!(
                "bitset_popcount CPU oracle failed to reserve {} output words: {err}. Fix: shard the bitset before parity evaluation.",
                input.len()
            )
        })?;
    }
    out.clear();
    out.extend(input.iter().map(|w| w.count_ones()));
    Ok(())
}

#[cfg(test)]
mod non_panic_wrapper_tests {
    use super::{cpu_ref, cpu_ref_into, try_cpu_ref_into};

    #[test]
    fn compatibility_wrappers_match_fallible_reference() {
        let input = [0, 0xFFFF_FFFF, 0xAAAA_AAAA, 0x8000_0001];
        let mut compat = Vec::with_capacity(8);
        let mut fallible = Vec::with_capacity(8);

        cpu_ref_into(&input, &mut compat);
        try_cpu_ref_into(&input, &mut fallible)
            .expect("Fix: small bitset_popcount CPU oracle must reserve");

        assert_eq!(cpu_ref(&input), fallible);
        assert_eq!(compat, fallible);
    }

    #[test]
    fn production_cpu_ref_wrappers_have_no_raw_panic_path() {
        let production = include_str!("popcount.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: popcount.rs must contain production section");

        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: bitset_popcount CPU parity wrappers must not panic in production."
        );
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || bitset_popcount("input", "count", 2),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0b1111, 0xFFFF_FFFF]), to_bytes(&[0, 0])]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[4, 32])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popcount_per_word() {
        assert_eq!(cpu_ref(&[0b1111, 0xFFFF_FFFF]), vec![4, 32]);
    }

    #[test]
    fn popcount_into_reuses_output() {
        let mut out = Vec::with_capacity(4);
        cpu_ref_into(&[0b1111, 0xFFFF_FFFF], &mut out);
        let capacity = out.capacity();
        assert_eq!(out, vec![4, 32]);

        cpu_ref_into(&[0b1010], &mut out);
        assert_eq!(out.capacity(), capacity);
        assert_eq!(out, vec![2]);
    }

    #[test]
    fn popcount_into_truncates_stale_tail_without_reallocating() {
        let mut out = Vec::with_capacity(8);
        out.extend([99u32; 8]);
        let ptr = out.as_ptr();

        try_cpu_ref_into(&[0b1111, 0xFFFF_FFFF], &mut out).unwrap();

        assert_eq!(out, vec![4, 32]);
        assert_eq!(out.as_ptr(), ptr);
    }

    // ------------------------------------------------------------------
    // Adversarial fixtures  -  empty, all-zeros, all-ones, alternating, cross-word.
    // ------------------------------------------------------------------

    #[test]
    fn empty_bitset() {
        assert_eq!(cpu_ref(&[]), Vec::<u32>::new());
    }

    #[test]
    fn single_word_all_zeros() {
        assert_eq!(cpu_ref(&[0]), vec![0]);
    }

    #[test]
    fn single_word_all_ones() {
        assert_eq!(cpu_ref(&[0xFFFF_FFFF]), vec![32]);
    }

    #[test]
    fn alternating_pattern() {
        // 0xAAAA_AAAA = 1010...1010 → 16 ones
        assert_eq!(cpu_ref(&[0xAAAA_AAAA]), vec![16]);
        // 0x5555_5555 = 0101...0101 → 16 ones
        assert_eq!(cpu_ref(&[0x5555_5555]), vec![16]);
    }

    #[test]
    fn cross_word_boundary() {
        // Two words: one with bit 31 set, one with bit 0 set.
        assert_eq!(cpu_ref(&[0x8000_0000, 0x0000_0001]), vec![1, 1]);
    }

    #[test]
    fn generated_popcount_matches_scalar_reference() {
        for len in 0..96usize {
            let input: Vec<u32> = (0..len)
                .map(|idx| {
                    (idx as u32)
                        .wrapping_mul(0x85EB_CA6B)
                        .wrapping_add(len as u32)
                })
                .collect();
            let mut out = Vec::with_capacity(len + 3);

            try_cpu_ref_into(&input, &mut out).unwrap();

            assert_eq!(
                out,
                input
                    .iter()
                    .map(|word| word.count_ones())
                    .collect::<Vec<_>>(),
                "generated popcount case len={len}"
            );
        }
    }
}
