//! `bitset_equal`  -  exact-equality check, writes 1 to `out_scalar`
//! iff every word of `lhs` equals the corresponding word of `rhs`.
//!
//! Used by fixpoint convergence checks: "did the frontier change?"
//! is `bitset_equal(prev, current, out_scalar)` then "if out == 1 stop."

use vyre_foundation::ir::Program;

use crate::bitset::relation::{bitset_relation_program, BitsetRelation};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::bitset::equal";

/// Build a Program: `out_scalar[0] = (forall w: lhs[w] == rhs[w]) ? 1 : 0`.
///
/// One-dispatch reduction: lane 0 initializes the output to true, then
/// every lane scans its chunk-strided words and atomically ANDs its
/// equality predicate into the scalar.
#[must_use]
pub fn bitset_equal(lhs: &str, rhs: &str, out_scalar: &str, words: u32) -> Program {
    bitset_relation_program(OP_ID, lhs, rhs, out_scalar, words, BitsetRelation::Equal)
}

/// Return whether `program` advertises the canonical `bitset_equal` op id.
///
/// Consumer crates should use this semantic tag helper instead of inspecting
/// the raw IR entry shape.
#[must_use]
pub fn is_bitset_equal_program(program: &Program) -> bool {
    if program.entry_op_id.as_deref() == Some(OP_ID) {
        return true;
    }
    matches!(
        program.entry.as_slice(),
        [vyre_foundation::ir::Node::Region { generator, .. }] if generator.as_ref() == OP_ID
    )
}

/// CPU reference: returns 1 iff every word matches, 0 otherwise.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn cpu_ref(lhs: &[u32], rhs: &[u32]) -> u32 {
    if lhs.len() != rhs.len() {
        return 0;
    }
    if lhs.iter().zip(rhs.iter()).all(|(a, b)| a == b) {
        1
    } else {
        0
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || bitset_equal("lhs", "rhs", "out", 2),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0xFFFF, 0xF0F0]),
                to_bytes(&[0xFFFF, 0xF0F0]),
                to_bytes(&[0]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[1])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::Node;

    #[test]
    fn identical_returns_one() {
        assert_eq!(cpu_ref(&[0xDEAD, 0xBEEF], &[0xDEAD, 0xBEEF]), 1);
    }

    #[test]
    fn differs_in_first_word_returns_zero() {
        assert_eq!(cpu_ref(&[0xDEAD, 0xBEEF], &[0xDEAE, 0xBEEF]), 0);
    }

    #[test]
    fn differs_in_last_word_returns_zero() {
        assert_eq!(cpu_ref(&[0, 0, 1], &[0, 0, 0]), 0);
    }

    #[test]
    fn empty_pair_returns_one() {
        assert_eq!(cpu_ref(&[], &[]), 1);
    }

    #[test]
    fn length_mismatch_returns_zero() {
        assert_eq!(cpu_ref(&[0], &[0, 0]), 0);
    }

    #[test]
    fn preserves_wrapper_op_id() {
        let program = bitset_equal("lhs", "rhs", "out", 2);
        let generator = match &program.entry[0] {
            Node::Region { generator, .. } => generator.to_string(),
            other => panic!("Fix: bitset_equal must build a Region entry, got {other:?}."),
        };
        assert_eq!(generator, OP_ID);
        assert!(is_bitset_equal_program(&program));
    }

    #[test]
    fn generated_adversarial_pairs_match_exact_equality_contract() {
        let mut state = 0xA5A5_5A5A_u32;
        for case in 0..4096 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let len = (state as usize % 17) + 1;
            let mut lhs = Vec::with_capacity(len);
            let mut rhs = Vec::with_capacity(len);
            for word in 0..len {
                state = state.rotate_left(5) ^ (case as u32).wrapping_mul(0x9E37_79B9);
                let value = state ^ (word as u32).wrapping_mul(0x85EB_CA6B);
                lhs.push(value);
                rhs.push(if case % 4 == 0 {
                    value
                } else {
                    value ^ (1_u32 << ((case + word) & 31))
                });
            }
            let expected = u32::from(lhs == rhs);
            assert_eq!(cpu_ref(&lhs, &rhs), expected, "case {case}");
        }
    }
}
