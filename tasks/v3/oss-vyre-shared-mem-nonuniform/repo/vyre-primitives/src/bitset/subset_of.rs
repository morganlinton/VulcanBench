//! `bitset_subset_of`  -  write 1 to `out_scalar` iff `lhs ⊆ rhs`.
//!
//! Equivalent: `(lhs & !rhs) == 0` per word for every word.

use vyre_foundation::ir::Program;

use crate::bitset::relation::{bitset_relation_program, BitsetRelation};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::bitset::subset_of";

/// Build a Program: `out_scalar[0] = (forall w: (lhs[w] & !rhs[w]) == 0) ? 1 : 0`.
#[must_use]
pub fn bitset_subset_of(lhs: &str, rhs: &str, out_scalar: &str, words: u32) -> Program {
    bitset_relation_program(OP_ID, lhs, rhs, out_scalar, words, BitsetRelation::SubsetOf)
}

/// CPU reference.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn cpu_ref(lhs: &[u32], rhs: &[u32]) -> u32 {
    let n = lhs.len().min(rhs.len());
    for i in 0..n {
        if (lhs[i] & !rhs[i]) != 0 {
            return 0;
        }
    }
    if lhs.len() > rhs.len() {
        for &word in &lhs[n..] {
            if word != 0 {
                return 0;
            }
        }
    }
    1
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || bitset_subset_of("lhs", "rhs", "out", 2),
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
    fn proper_subset_returns_one() {
        assert_eq!(cpu_ref(&[0b0011], &[0b1111]), 1);
    }

    #[test]
    fn equal_sets_are_subsets() {
        assert_eq!(cpu_ref(&[0xDEAD], &[0xDEAD]), 1);
    }

    #[test]
    fn superset_returns_zero() {
        assert_eq!(cpu_ref(&[0b1111], &[0b0011]), 0);
    }

    #[test]
    fn disjoint_nonempty_returns_zero() {
        assert_eq!(cpu_ref(&[0b1100], &[0b0011]), 0);
    }

    #[test]
    fn empty_lhs_is_subset_of_anything() {
        assert_eq!(cpu_ref(&[0], &[0xFFFF_FFFF]), 1);
    }

    #[test]
    fn preserves_wrapper_op_id() {
        let program = bitset_subset_of("lhs", "rhs", "out", 2);
        let generator = match &program.entry[0] {
            Node::Region { generator, .. } => generator.to_string(),
            other => panic!("Fix: bitset_subset_of must build a Region entry, got {other:?}."),
        };
        assert_eq!(generator, OP_ID);
    }

    #[test]
    fn generated_adversarial_pairs_match_subset_contract() {
        let mut state = 0xC001_D00D_u32;
        for case in 0..4096 {
            state = state.wrapping_mul(22_695_477).wrapping_add(1);
            let len = (state as usize % 19) + 1;
            let mut lhs = Vec::with_capacity(len);
            let mut rhs = Vec::with_capacity(len);
            for word in 0..len {
                state = state.rotate_left(7) ^ (word as u32).wrapping_mul(0x27D4_EB2D);
                let superset = state;
                let candidate = if case % 3 == 0 {
                    superset & state.rotate_right((case + word) as u32 & 31)
                } else {
                    superset ^ (1_u32 << ((case + word * 3) & 31))
                };
                lhs.push(candidate);
                rhs.push(superset);
            }
            let expected = u32::from(lhs.iter().zip(&rhs).all(|(a, b)| (a & !b) == 0));
            assert_eq!(cpu_ref(&lhs, &rhs), expected, "case {case}");
        }
    }
}
