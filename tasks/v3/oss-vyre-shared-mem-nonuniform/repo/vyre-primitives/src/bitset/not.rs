//! `bitset_not`  -  per-word bitwise NOT over a packed bitset.

use vyre_foundation::ir::{Program, UnOp};

use crate::bitset::unary_word::bitset_unary_word_program;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::bitset::not";

/// Build a Program: `out[w] = !input[w]`.
///
/// Per-word bitwise complement for boolean-algebra lowering when a negated
/// predicate needs a complement bitset for a subsequent intersection.
///
/// # Example
///
/// ```
/// use vyre_primitives::bitset::not::bitset_not;
///
/// let program = bitset_not("input", "out", 4);
/// assert_eq!(program.entry.len(), 1);
/// ```
#[must_use]
pub fn bitset_not(input: &str, out: &str, words: u32) -> Program {
    bitset_unary_word_program(OP_ID, input, out, words, UnOp::BitNot)
}

/// CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(input: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    match try_cpu_ref_into(input, &mut out) {
        Ok(()) => out,
        // A parity oracle that returns empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a real divergence
        // (Law 10 / Law 6). Fail loud; callers use try_cpu_ref_into.
        Err(error) => panic!("vyre-primitives bitset_not cpu_ref failed: {error}"),
    }
}

/// CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(input: &[u32], out: &mut Vec<u32>) {
    if let Err(error) = try_cpu_ref_into(input, out) {
        panic!("vyre-primitives bitset_not cpu_ref_into failed: {error}");
    }
}

/// Fallible CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(input: &[u32], out: &mut Vec<u32>) -> Result<(), String> {
    if input.len() > out.capacity() {
        out.try_reserve(input.len() - out.len()).map_err(|err| {
            format!(
                "bitset_not CPU oracle failed to reserve {} output words: {err}. Fix: shard the bitset before parity evaluation.",
                input.len()
            )
        })?;
    }
    out.clear();
    out.extend(input.iter().map(|word| !word));
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || bitset_not("input", "out", 1),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0x0F0F_0F0F]), to_bytes(&[0])]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0xF0F0_F0F0])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flips_every_bit() {
        assert_eq!(cpu_ref(&[0x0F0F_0F0F]), vec![0xF0F0_F0F0]);
    }

    #[test]
    fn empty_bitset() {
        assert_eq!(cpu_ref(&[]), Vec::<u32>::new());
    }

    #[test]
    fn single_word_all_bits() {
        assert_eq!(cpu_ref(&[0xFFFF_FFFF]), vec![0x0000_0000]);
        assert_eq!(cpu_ref(&[0x0000_0000]), vec![0xFFFF_FFFF]);
    }

    #[test]
    fn cross_word_boundary() {
        let input = vec![0x8000_0000, 0x0000_0001];
        assert_eq!(cpu_ref(&input), vec![0x7FFF_FFFF, 0xFFFF_FFFE]);
    }

    #[test]
    fn cpu_ref_into_truncates_stale_tail_without_reallocating() {
        let input = vec![0x8000_0000, 0x0000_0001];
        let mut out = Vec::with_capacity(8);
        out.extend([0xDEAD_BEEF; 8]);
        let ptr = out.as_ptr();

        try_cpu_ref_into(&input, &mut out).unwrap();

        assert_eq!(out, vec![0x7FFF_FFFF, 0xFFFF_FFFE]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn compatibility_wrappers_match_fallible_reference() {
        let input = [0x1234_5678, 0xFFFF_0000];
        let mut compat = Vec::with_capacity(4);
        let mut fallible = Vec::with_capacity(4);

        cpu_ref_into(&input, &mut compat);
        try_cpu_ref_into(&input, &mut fallible)
            .expect("Fix: small bitset_not CPU oracle must reserve");

        assert_eq!(cpu_ref(&input), fallible);
        assert_eq!(compat, fallible);
    }

    #[test]
    fn production_cpu_ref_wrappers_have_no_raw_panic_path() {
        let production = include_str!("not.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: not.rs must contain production section");

        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: bitset_not CPU parity wrappers must not use bare .unwrap()/.expect() — use an explicit panic!() with the error."
        );
        assert!(
            !production.contains(concat!("eprintln", "!(\"vyre-primitives bitset_not")),
            "Fix: bitset_not CPU oracle must not log-and-return empty on error (silently masks a parity divergence, Law 10) — fail loud via panic!() so callers use try_cpu_ref_into."
        );
        assert!(
            production.contains("panic!("),
            "Fix: bitset_not CPU oracle must panic!() when it cannot compute the reference, never return an empty vec."
        );
    }

    #[test]
    fn generated_not_is_involutive_and_matches_scalar_reference() {
        for len in 0..96usize {
            let input: Vec<u32> = (0..len)
                .map(|idx| {
                    (idx as u32)
                        .wrapping_mul(0x9E37_79B9)
                        .wrapping_add(len as u32)
                })
                .collect();
            let mut out = Vec::with_capacity(len + 3);
            let mut roundtrip = Vec::with_capacity(len + 3);

            try_cpu_ref_into(&input, &mut out).unwrap();
            try_cpu_ref_into(&out, &mut roundtrip).unwrap();

            assert_eq!(
                out,
                input.iter().map(|word| !word).collect::<Vec<_>>(),
                "generated not case len={len}"
            );
            assert_eq!(roundtrip, input, "generated involution case len={len}");
        }
    }
}
