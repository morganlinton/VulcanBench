//! Functorial composition helpers for optimizer pass rows.

#![allow(deprecated)]

use crate::cpu_references::functor_apply_cpu;

/// Apply a pass functor mapping to a row vector.
#[must_use]
pub fn apply_pass_functor(values: &[u32], mapping: &[u32], target_size: u32) -> Vec<u32> {
    functor_apply_cpu(values, mapping, target_size)
}

/// Compose two pass mappings against `values`.
#[must_use]
pub fn compose_passes(
    values: &[u32],
    first_mapping: &[u32],
    first_size: u32,
    second_mapping: &[u32],
    output_size: u32,
) -> Vec<u32> {
    let intermediate = apply_pass_functor(values, first_mapping, first_size);
    apply_pass_functor(&intermediate, second_mapping, output_size)
}

/// Identity mapping for `n` pass slots.
#[must_use]
pub fn identity_functor(n: u32) -> Vec<u32> {
    (0..n).collect()
}

/// Return whether two pass paths produce the same row mapping.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn passes_commute_on(
    values: &[u32],
    left_first: &[u32],
    left_mid_size: u32,
    left_second: &[u32],
    right_first: &[u32],
    right_mid_size: u32,
    right_second: &[u32],
    output_size: u32,
) -> bool {
    compose_passes(values, left_first, left_mid_size, left_second, output_size)
        == compose_passes(
            values,
            right_first,
            right_mid_size,
            right_second,
            output_size,
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_functor_is_0_to_n() {
        assert_eq!(identity_functor(4), vec![0, 1, 2, 3]);
    }

    #[test]
    fn identity_functor_zero() {
        assert!(identity_functor(0).is_empty());
    }

    #[test]
    fn apply_identity_preserves_values() {
        let values = vec![10, 20, 30];
        let mapping = identity_functor(3);
        let result = apply_pass_functor(&values, &mapping, 3);
        assert_eq!(result, vec![10, 20, 30]);
    }

    #[test]
    fn compose_two_identity_passes() {
        let values = vec![5, 10];
        let id = identity_functor(2);
        let result = compose_passes(&values, &id, 2, &id, 2);
        assert_eq!(result, values);
    }

    #[test]
    fn compose_with_permutation() {
        let values = vec![1, 2, 3];
        let id = identity_functor(3);
        let reverse = vec![2u32, 1, 0]; // reverse mapping
        let result = compose_passes(&values, &id, 3, &reverse, 3);
        assert_eq!(result, vec![3, 2, 1]);
    }

    #[test]
    fn identity_passes_commute() {
        let values = vec![1, 2, 3];
        let id = identity_functor(3);
        assert!(passes_commute_on(
            &values, &id, 3, &id, // left path: id ∘ id
            &id, 3, &id, // right path: id ∘ id
            3
        ));
    }
}
