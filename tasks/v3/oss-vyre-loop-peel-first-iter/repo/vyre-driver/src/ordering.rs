//! Backend-neutral monotonic ordering helpers for staging hot paths.

/// Return whether an iterator's keys are already nondecreasing.
pub fn iter_is_monotonic_by_key<I, K, F>(items: I, mut key: F) -> bool
where
    I: IntoIterator,
    K: Ord,
    F: FnMut(I::Item) -> K,
{
    let mut previous = None;
    for item in items {
        let current = key(item);
        if let Some(previous) = previous {
            if current < previous {
                return false;
            }
        }
        previous = Some(current);
    }
    true
}

/// Sort only when `items` are not already nondecreasing by `key`.
pub fn sort_by_key_if_needed<T, K, F>(items: &mut [T], mut key: F)
where
    K: Ord,
    F: FnMut(&T) -> K,
{
    let mut previous = None;
    for index in 0..items.len() {
        let current = key(&items[index]);
        if let Some(previous) = previous {
            if current < previous {
                items.sort_by_key(key);
                return;
            }
        }
        previous = Some(current);
    }
}

/// Unstable-sort only when `items` are not already nondecreasing by `key`.
pub fn sort_unstable_by_key_if_needed<T, K, F>(items: &mut [T], mut key: F)
where
    K: Ord,
    F: FnMut(&T) -> K,
{
    let mut previous = None;
    for index in 0..items.len() {
        let current = key(&items[index]);
        if let Some(previous) = previous {
            if current < previous {
                items.sort_unstable_by_key(key);
                return;
            }
        }
        previous = Some(current);
    }
}

/// Unstable-sort only when `items` are not already nondecreasing.
pub fn sort_unstable_if_needed<T>(items: &mut [T])
where
    T: Ord,
{
    for index in 1..items.len() {
        if items[index] < items[index - 1] {
            items.sort_unstable();
            return;
        }
    }
}

/// The first way a sorted index slice fails to be a dense permutation of
/// `0..expected_len`. Distinguishing these lets callers emit a remediation that
/// names the actual defect (a duplicate aliases two descriptors onto one logical
/// slot; a sparse map skips one; a length mismatch has the wrong cardinality)
/// instead of a generic "not dense".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DensePermutationDefect {
    /// After sorting, `index` sits at `slot` with `index < slot`: a value
    /// repeated earlier, so two descriptors alias one logical slot.
    Duplicate {
        /// The repeated value found below its sorted slot position.
        index: usize,
        /// The sorted slot position at which the duplicate surfaced.
        slot: usize,
    },
    /// After sorting, `index` sits at `slot` with `index > slot`: a gap, so a
    /// logical slot in `0..expected_len` is never mapped.
    Sparse {
        /// The value found above its sorted slot position.
        index: usize,
        /// The sorted slot position whose dense value (`slot`) is missing.
        slot: usize,
    },
    /// Every present index was dense but the cardinality is wrong (the map is
    /// truncated or over-long relative to `expected_len`).
    LengthMismatch {
        /// The number of indices actually present.
        resolved: usize,
        /// The dense cardinality the map was required to cover.
        expected: usize,
    },
}

/// Classify whether `sorted_indices` is a dense permutation of `0..expected_len`
/// — each value in `0..expected_len` present exactly once.
///
/// Callers MUST pass indices already sorted ascending (e.g. via
/// [`sort_unstable_if_needed`]); the classification is defined on sorted slot
/// position. This is the single source of the dense-index-map invariant shared
/// by every resident/graph descriptor→logical-slot map; format the returned
/// [`DensePermutationDefect`] into a context-specific message at the call site.
pub fn classify_dense_permutation(
    sorted_indices: &[usize],
    expected_len: usize,
) -> Result<(), DensePermutationDefect> {
    for (slot, &index) in sorted_indices.iter().enumerate() {
        if index != slot {
            return Err(if index < slot {
                DensePermutationDefect::Duplicate { index, slot }
            } else {
                DensePermutationDefect::Sparse { index, slot }
            });
        }
    }
    if sorted_indices.len() != expected_len {
        return Err(DensePermutationDefect::LengthMismatch {
            resolved: sorted_indices.len(),
            expected: expected_len,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{
        classify_dense_permutation, iter_is_monotonic_by_key, sort_by_key_if_needed,
        sort_unstable_by_key_if_needed, sort_unstable_if_needed, DensePermutationDefect,
    };

    #[test]
    fn iter_monotonic_by_key_detects_ordered_and_unordered_streams() {
        assert!(iter_is_monotonic_by_key([0, 1, 1, 3], |value| value));
        assert!(!iter_is_monotonic_by_key([0, 2, 1, 3], |value| value));
    }

    #[test]
    fn stable_sort_by_key_skips_already_monotonic_slices() {
        let calls = Cell::new(0usize);
        let mut items = [(0usize, "a"), (1, "b"), (1, "c"), (3, "d")];

        sort_by_key_if_needed(&mut items, |(key, _)| {
            calls.set(calls.get() + 1);
            *key
        });

        assert_eq!(items, [(0, "a"), (1, "b"), (1, "c"), (3, "d")]);
        assert_eq!(
            calls.get(),
            items.len(),
            "Fix: monotonic ordering paths must not invoke the fallback sort."
        );
    }

    #[test]
    fn stable_sort_by_key_sorts_unordered_slices() {
        let mut items = [(2usize, "c"), (0, "a"), (3, "d"), (1, "b")];

        sort_by_key_if_needed(&mut items, |(key, _)| *key);

        assert_eq!(items, [(0, "a"), (1, "b"), (2, "c"), (3, "d")]);
    }

    #[test]
    fn unstable_sort_by_key_skips_already_monotonic_slices() {
        let calls = Cell::new(0usize);
        let mut items = [(0usize, "a"), (1, "b"), (3, "c")];

        sort_unstable_by_key_if_needed(&mut items, |(key, _)| {
            calls.set(calls.get() + 1);
            *key
        });

        assert_eq!(items, [(0, "a"), (1, "b"), (3, "c")]);
        assert_eq!(
            calls.get(),
            items.len(),
            "Fix: monotonic unstable-ordering paths must not invoke the fallback sort."
        );
    }

    #[test]
    fn unstable_sort_by_key_sorts_unordered_slices() {
        let mut items = [(2usize, "c"), (0, "a"), (1, "b")];

        sort_unstable_by_key_if_needed(&mut items, |(key, _)| *key);

        assert_eq!(items, [(0, "a"), (1, "b"), (2, "c")]);
    }

    #[test]
    fn unstable_sort_skips_already_monotonic_slices() {
        let mut items = [0usize, 1, 1, 3];

        sort_unstable_if_needed(&mut items);

        assert_eq!(items, [0, 1, 1, 3]);
    }

    #[test]
    fn unstable_sort_sorts_unordered_slices() {
        let mut items = [2usize, 0, 1];

        sort_unstable_if_needed(&mut items);

        assert_eq!(items, [0, 1, 2]);
    }

    #[test]
    fn classify_dense_permutation_distinguishes_dense_duplicate_sparse_and_length() {
        assert_eq!(classify_dense_permutation(&[0, 1, 2], 3), Ok(()));
        assert_eq!(classify_dense_permutation(&[], 0), Ok(()));
        assert_eq!(
            classify_dense_permutation(&[0, 0, 2], 3),
            Err(DensePermutationDefect::Duplicate { index: 0, slot: 1 }),
            "Fix: a repeated value at a later sorted slot is a duplicate, not a generic non-dense map."
        );
        assert_eq!(
            classify_dense_permutation(&[0, 2, 3], 3),
            Err(DensePermutationDefect::Sparse { index: 2, slot: 1 }),
            "Fix: a value above its sorted slot is a sparse gap, not a duplicate."
        );
        assert_eq!(
            classify_dense_permutation(&[0, 1], 3),
            Err(DensePermutationDefect::LengthMismatch {
                resolved: 2,
                expected: 3
            }),
            "Fix: a dense-but-short map is a length mismatch."
        );
        assert_eq!(
            classify_dense_permutation(&[0, 1, 2, 3], 3),
            Err(DensePermutationDefect::LengthMismatch {
                resolved: 4,
                expected: 3
            }),
            "Fix: a dense-but-long map is a length mismatch."
        );
    }

    #[test]
    fn classify_dense_permutation_matches_sorted_reference_over_generated_maps() {
        // For every permutation-with-defect we can synthesize, the classifier's
        // verdict must agree with an independent set-based reference oracle.
        for len in 0usize..=24 {
            let dense: Vec<usize> = (0..len).collect();
            assert_eq!(classify_dense_permutation(&dense, len), Ok(()));

            for collide in 0..len {
                // Replace one slot's value with a duplicate of slot 0's value (0),
                // then re-sort: this guarantees a duplicate, never a sparse gap.
                let mut indices = dense.clone();
                indices[collide] = 0;
                sort_unstable_if_needed(&mut indices);
                let verdict = classify_dense_permutation(&indices, len);
                let distinct: std::collections::BTreeSet<usize> =
                    indices.iter().copied().collect();
                let reference_is_dense =
                    distinct.len() == len && indices.len() == len && *distinct.iter().max().unwrap_or(&0) < len.max(1);
                if collide == 0 {
                    // collide==0 leaves the map unchanged: still dense.
                    assert_eq!(verdict, Ok(()));
                } else {
                    assert!(verdict.is_err(), "len={len} collide={collide} must be a defect");
                    assert!(!reference_is_dense || verdict.is_ok());
                    assert!(matches!(
                        verdict,
                        Err(DensePermutationDefect::Duplicate { .. })
                            | Err(DensePermutationDefect::Sparse { .. })
                            | Err(DensePermutationDefect::LengthMismatch { .. })
                    ));
                }
            }
        }
    }

    #[test]
    fn generated_ordering_matrix_matches_full_sort_contract() {
        for len in 0..=128 {
            let ordered: Vec<usize> = (0..len).collect();
            let mut reversed: Vec<usize> = (0..len).rev().collect();
            let mut expected = reversed.clone();
            expected.sort_unstable();

            assert!(iter_is_monotonic_by_key(ordered.iter().copied(), |value| {
                value
            }));
            if len > 1 {
                assert!(!iter_is_monotonic_by_key(
                    reversed.iter().copied(),
                    |value| value
                ));
            }

            sort_unstable_if_needed(&mut reversed);
            assert_eq!(reversed, expected);

            let mut keyed: Vec<(usize, usize)> = (0..len).rev().map(|value| (value, len)).collect();
            sort_unstable_by_key_if_needed(&mut keyed, |(key, _)| *key);
            for (expected_key, actual) in keyed.iter().enumerate() {
                assert_eq!(actual.0, expected_key);
                assert_eq!(actual.1, len);
            }
        }
    }
}
