//! CUDA-facing re-exports of backend-neutral monotonic ordering helpers.

pub(crate) use vyre_driver::ordering::{
    classify_dense_permutation, sort_unstable_by_key_if_needed, sort_unstable_if_needed,
    DensePermutationDefect,
};

#[cfg(test)]
mod tests {
    use super::{sort_unstable_by_key_if_needed, sort_unstable_if_needed};

    const SOURCE: &str = include_str!("ordering.rs");

    #[test]
    fn cuda_ordering_module_is_reexport_not_sort_fork() {
        assert!(
            SOURCE.contains("pub(crate) use vyre_driver::ordering"),
            "CUDA ordering must bind to the backend-neutral owner"
        );
        let direct_sort = concat!(".sort_", "unstable(");
        let direct_key_sort = concat!(".sort_", "unstable_by_key(");
        assert!(
            !SOURCE.contains(direct_sort) && !SOURCE.contains(direct_key_sort),
            "CUDA ordering must not reimplement monotonic sorting"
        );
    }

    #[test]
    fn cuda_ordering_reexport_matches_owner_on_generated_reverse_inputs() {
        for len in 0..=128 {
            let mut values: Vec<usize> = (0..len).rev().collect();
            let expected: Vec<usize> = (0..len).collect();
            sort_unstable_if_needed(&mut values);
            assert_eq!(values, expected);

            let mut keyed: Vec<(usize, usize)> = (0..len).rev().map(|value| (value, len)).collect();
            sort_unstable_by_key_if_needed(&mut keyed, |(key, _)| *key);
            for (expected_key, actual) in keyed.iter().enumerate() {
                assert_eq!(actual.0, expected_key);
                assert_eq!(actual.1, len);
            }
        }
    }
}
